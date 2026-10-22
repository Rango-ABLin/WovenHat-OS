//! WovenWiFi Stage 13.10K - firmware image ownership/validation foundation.
//!
//! Bounded image ownership, Intel TLV container validation, DMA staging and
//! the AX201 API-77 startup/command boundary. Container parsing is shared
//! with host tests through `wifi_tlv.rs`; physical radio qualification still
//! requires device-produced evidence.

#[path = "wifi_firmware_tlv.rs"]
mod tlv;
pub use tlv::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelFirmwareProfileError {
    UnsupportedFamily,
    Filename { expected: &'static str },
    ApiVersion { found: u32, min: u32, max: u32 },
    RuntimeImage(Ax200ImageError),
}

/// Firmware supplied by the platform's provider (filesystem, boot module or
/// a build-time include). The bytes remain borrowed only for parsing; the
/// driver copies all device-visible sections into owned DMA before returning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201FirmwareSource<'a> {
    pub filename: &'a str,
    pub bytes: &'a [u8],
}

pub fn ax201_firmware_filename(
    function: crate::wifi_hw::SupportedWifiFunction,
) -> Option<&'static str> {
    if function.family != crate::wifi_hw::WifiDriverFamily::IntelIwlwifiAx201
        || function.firmware.api_min != function.firmware.api_max
    {
        return None;
    }
    Some("iwlwifi-QuZ-a0-hr-b0-77.ucode")
}

/// Verify that a parsed Intel container belongs to the selected 22000-family
/// device before allocating any DMA backing. AX201 firmware is selected by
/// the PCI identity/profile, never by accepting an arbitrary filename or by
/// reusing the AX200 profile.
pub fn validate_firmware_profile(
    function: crate::wifi_hw::SupportedWifiFunction,
    firmware: &IntelTlvFirmware<'_>,
) -> Result<(), IntelFirmwareProfileError> {
    if !matches!(
        function.family,
        crate::wifi_hw::WifiDriverFamily::IntelIwlwifi
            | crate::wifi_hw::WifiDriverFamily::IntelIwlwifiAx201
    ) {
        return Err(IntelFirmwareProfileError::UnsupportedFamily);
    }
    let api = firmware.api_version();
    if api < function.firmware.api_min || api > function.firmware.api_max {
        return Err(IntelFirmwareProfileError::ApiVersion {
            found: api,
            min: function.firmware.api_min,
            max: function.firmware.api_max,
        });
    }
    Ax200RuntimeImage::validate(firmware)
        .map(|_| ())
        .map_err(IntelFirmwareProfileError::RuntimeImage)
}

pub fn validate_ax201_firmware_source(
    function: crate::wifi_hw::SupportedWifiFunction,
    source: Ax201FirmwareSource<'_>,
) -> Result<(), Intel22000FirmwarePrepareError> {
    let expected = ax201_firmware_filename(function).ok_or(
        Intel22000FirmwarePrepareError::Profile(IntelFirmwareProfileError::UnsupportedFamily),
    )?;
    if source.filename != expected {
        return Err(Intel22000FirmwarePrepareError::Profile(
            IntelFirmwareProfileError::Filename { expected },
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intel22000FirmwarePrepareError {
    Parse(IntelTlvError),
    Profile(IntelFirmwareProfileError),
    Context(IntelContextInfoBuildError),
}

/// Parse, profile-check and copy an external AX201 firmware container into
/// kernel-owned DMA buffers. The caller must retain no borrowed firmware after
/// this function returns; the context owns every staged payload needed for the
/// later hardware publication step.
pub fn prepare_ax201_runtime_firmware(
    function: crate::wifi_hw::SupportedWifiFunction,
    bytes: &[u8],
) -> Result<Intel22000DmaContextInfo, Intel22000FirmwarePrepareError> {
    if function.family != crate::wifi_hw::WifiDriverFamily::IntelIwlwifiAx201 {
        return Err(Intel22000FirmwarePrepareError::Profile(
            IntelFirmwareProfileError::UnsupportedFamily,
        ));
    }
    let firmware = IntelTlvFirmware::parse(bytes).map_err(Intel22000FirmwarePrepareError::Parse)?;
    validate_firmware_profile(function, &firmware)
        .map_err(Intel22000FirmwarePrepareError::Profile)?;
    Intel22000DmaContextInfo::from_runtime_firmware(&firmware)
        .map_err(Intel22000FirmwarePrepareError::Context)
}

pub fn prepare_ax201_runtime_firmware_from_source(
    function: crate::wifi_hw::SupportedWifiFunction,
    source: Ax201FirmwareSource<'_>,
) -> Result<Intel22000DmaContextInfo, Intel22000FirmwarePrepareError> {
    validate_ax201_firmware_source(function, source)?;
    prepare_ax201_runtime_firmware(function, source.bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareSectionKind {
    Instruction,
    Data,
    Paging,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirmwareSection<'a> {
    pub kind: FirmwareSectionKind,
    pub device_offset: u32,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareError {
    EmptyImage,
    ImageTooLarge,
    NoSections,
    TooManySections,
    EmptySection,
    SectionTooLarge,
    SectionOutsideImage,
    AddressOverflow,
    OverlappingDeviceRange,
    InvalidState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareLoadState {
    Empty,
    Validated,
    Loading,
    Loaded,
    Failed,
}

/// Borrowed, validated firmware image.
///
/// Stage K does not allocate or copy firmware. The image and every section are
/// borrowed, so the caller must keep the source bytes alive for this value's
/// lifetime. Later hardware-loading stages can add owned/staged DMA buffers at
/// the boundary where device transfer actually begins.
pub struct FirmwareImage<'a> {
    bytes: &'a [u8],
    sections: &'a [FirmwareSection<'a>],
}

impl<'a> FirmwareImage<'a> {
    pub fn validate(
        bytes: &'a [u8],
        sections: &'a [FirmwareSection<'a>],
    ) -> Result<Self, FirmwareError> {
        if bytes.is_empty() {
            return Err(FirmwareError::EmptyImage);
        }
        if bytes.len() > MAX_FIRMWARE_IMAGE_SIZE {
            return Err(FirmwareError::ImageTooLarge);
        }
        if sections.is_empty() {
            return Err(FirmwareError::NoSections);
        }
        if sections.len() > MAX_FIRMWARE_SECTIONS {
            return Err(FirmwareError::TooManySections);
        }

        let image_start = bytes.as_ptr() as usize;
        let image_end = image_start
            .checked_add(bytes.len())
            .ok_or(FirmwareError::SectionOutsideImage)?;

        for (index, section) in sections.iter().enumerate() {
            if section.bytes.is_empty() {
                return Err(FirmwareError::EmptySection);
            }
            if section.bytes.len() > MAX_FIRMWARE_SECTION_SIZE {
                return Err(FirmwareError::SectionTooLarge);
            }

            let section_start = section.bytes.as_ptr() as usize;
            let section_end = section_start
                .checked_add(section.bytes.len())
                .ok_or(FirmwareError::SectionOutsideImage)?;
            if section_start < image_start || section_end > image_end {
                return Err(FirmwareError::SectionOutsideImage);
            }

            let length =
                u32::try_from(section.bytes.len()).map_err(|_| FirmwareError::AddressOverflow)?;
            let section_device_end = section
                .device_offset
                .checked_add(length)
                .ok_or(FirmwareError::AddressOverflow)?;

            for previous in &sections[..index] {
                let previous_length = u32::try_from(previous.bytes.len())
                    .map_err(|_| FirmwareError::AddressOverflow)?;
                let previous_end = previous
                    .device_offset
                    .checked_add(previous_length)
                    .ok_or(FirmwareError::AddressOverflow)?;
                if section.device_offset < previous_end
                    && previous.device_offset < section_device_end
                {
                    return Err(FirmwareError::OverlappingDeviceRange);
                }
            }
        }

        Ok(Self { bytes, sections })
    }

    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    pub const fn section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn section(&self, index: usize) -> Option<FirmwareSection<'a>> {
        self.sections.get(index).copied()
    }
}

pub struct FirmwareLoadSession<'a> {
    image: FirmwareImage<'a>,
    state: FirmwareLoadState,
    next_section: usize,
}

impl<'a> FirmwareLoadSession<'a> {
    pub fn new(image: FirmwareImage<'a>) -> Self {
        Self {
            image,
            state: FirmwareLoadState::Validated,
            next_section: 0,
        }
    }

    pub const fn state(&self) -> FirmwareLoadState {
        self.state
    }

    pub const fn next_section_index(&self) -> usize {
        self.next_section
    }

    pub fn begin(&mut self) -> Result<(), FirmwareError> {
        if self.state != FirmwareLoadState::Validated {
            return Err(FirmwareError::InvalidState);
        }
        self.state = FirmwareLoadState::Loading;
        Ok(())
    }

    /// Mark one already-transferred section complete.
    ///
    /// This method does not perform I/O. A later stage must call it only after
    /// the chipset-specific transfer primitive has completed successfully.
    pub fn complete_section(&mut self) -> Result<(), FirmwareError> {
        if self.state != FirmwareLoadState::Loading {
            return Err(FirmwareError::InvalidState);
        }
        if self.next_section >= self.image.section_count() {
            self.state = FirmwareLoadState::Failed;
            return Err(FirmwareError::InvalidState);
        }

        self.next_section += 1;
        if self.next_section == self.image.section_count() {
            self.state = FirmwareLoadState::Loaded;
        }
        Ok(())
    }

    pub fn fail(&mut self) {
        self.state = FirmwareLoadState::Failed;
    }
}

pub fn stage13_10k_self_test() -> bool {
    let bytes = [0x11u8; 96];
    let sections = [
        FirmwareSection {
            kind: FirmwareSectionKind::Instruction,
            device_offset: 0x1000,
            bytes: &bytes[0..32],
        },
        FirmwareSection {
            kind: FirmwareSectionKind::Data,
            device_offset: 0x2000,
            bytes: &bytes[32..64],
        },
        FirmwareSection {
            kind: FirmwareSectionKind::Paging,
            device_offset: 0x3000,
            bytes: &bytes[64..96],
        },
    ];

    let Ok(image) = FirmwareImage::validate(&bytes, &sections) else {
        return false;
    };
    if image.len() != 96
        || image.section_count() != 3
        || image.section(0) != Some(sections[0])
        || image.section(3).is_some()
    {
        return false;
    }

    let mut load = FirmwareLoadSession::new(image);
    if load.state() != FirmwareLoadState::Validated
        || load.complete_section() != Err(FirmwareError::InvalidState)
        || load.begin().is_err()
        || load.state() != FirmwareLoadState::Loading
        || load.complete_section().is_err()
        || load.next_section_index() != 1
        || load.complete_section().is_err()
        || load.next_section_index() != 2
        || load.complete_section().is_err()
        || load.state() != FirmwareLoadState::Loaded
        || load.complete_section() != Err(FirmwareError::InvalidState)
    {
        return false;
    }

    let overlap = [
        FirmwareSection {
            kind: FirmwareSectionKind::Instruction,
            device_offset: 0x1000,
            bytes: &bytes[0..32],
        },
        FirmwareSection {
            kind: FirmwareSectionKind::Data,
            device_offset: 0x1010,
            bytes: &bytes[32..64],
        },
    ];
    if !matches!(
        FirmwareImage::validate(&bytes, &overlap),
        Err(FirmwareError::OverlappingDeviceRange)
    ) {
        return false;
    }

    let external = [0x55u8; 8];
    let outside = [FirmwareSection {
        kind: FirmwareSectionKind::Data,
        device_offset: 0x4000,
        bytes: &external,
    }];
    if !matches!(
        FirmwareImage::validate(&bytes, &outside),
        Err(FirmwareError::SectionOutsideImage)
    ) {
        return false;
    }

    let empty_section = [FirmwareSection {
        kind: FirmwareSectionKind::Data,
        device_offset: 0x5000,
        bytes: &bytes[0..0],
    }];
    if !matches!(
        FirmwareImage::validate(&bytes, &empty_section),
        Err(FirmwareError::EmptySection)
    ) {
        return false;
    }

    let one = [FirmwareSection {
        kind: FirmwareSectionKind::Data,
        device_offset: 0xffff_fff0,
        bytes: &bytes[0..32],
    }];
    matches!(
        FirmwareImage::validate(&bytes, &one),
        Err(FirmwareError::AddressOverflow)
    )
}
fn read_le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = bytes.get(offset..end)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[path = "wifi_ax200_image.rs"]
mod ax200_image;
pub use ax200_image::{Ax200ImageError, Ax200ImageRegion, Ax200RuntimeImage};

pub fn stage13_10l_self_test() -> bool {
    let mut blob = [0u8; 176];
    blob[4..8].copy_from_slice(&INTEL_TLV_UCODE_MAGIC.to_le_bytes());
    blob[72..76].copy_from_slice(&0x1122_3344u32.to_le_bytes());
    blob[76..80].copy_from_slice(&7u32.to_le_bytes());

    // Runtime SEC TLV. Intel section payload begins with a LE device offset.
    let mut cursor = INTEL_TLV_UCODE_HEADER_SIZE;
    blob[cursor..cursor + 4].copy_from_slice(&INTEL_TLV_SEC_RT.to_le_bytes());
    blob[cursor + 4..cursor + 8].copy_from_slice(&12u32.to_le_bytes());
    blob[cursor + 8..cursor + 12].copy_from_slice(&0x2000u32.to_le_bytes());
    blob[cursor + 12..cursor + 20].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    cursor += 20;

    // Unknown TLV must be safely skipped, including 4-byte alignment padding.
    blob[cursor..cursor + 4].copy_from_slice(&0xfeedu32.to_le_bytes());
    blob[cursor + 4..cursor + 8].copy_from_slice(&3u32.to_le_bytes());
    blob[cursor + 8..cursor + 11].copy_from_slice(&[9, 8, 7]);
    cursor += 12;

    // Paging is metadata, not an addressed section. Exercise secure sections
    // and a CPU delimiter on the kernel target as well as in host fixtures.
    blob[cursor..cursor + 4].copy_from_slice(&INTEL_TLV_PAGING.to_le_bytes());
    blob[cursor + 4..cursor + 8].copy_from_slice(&4u32.to_le_bytes());
    blob[cursor + 8..cursor + 12].copy_from_slice(&4096u32.to_le_bytes());
    cursor += 12;
    blob[cursor..cursor + 4].copy_from_slice(&INTEL_TLV_SECURE_SEC_RT.to_le_bytes());
    blob[cursor + 4..cursor + 8].copy_from_slice(&4u32.to_le_bytes());
    blob[cursor + 8..cursor + 12].copy_from_slice(&INTEL_CPU_SEPARATOR.to_le_bytes());
    cursor += 12;
    blob[cursor..cursor + 4].copy_from_slice(&INTEL_TLV_SECURE_SEC_RT.to_le_bytes());
    blob[cursor + 4..cursor + 8].copy_from_slice(&8u32.to_le_bytes());
    blob[cursor + 8..cursor + 12].copy_from_slice(&0x3000u32.to_le_bytes());
    blob[cursor + 12..cursor + 16].copy_from_slice(&[5, 6, 7, 8]);
    cursor += 16;

    let Ok(parsed) = IntelTlvFirmware::parse(&blob[..cursor]) else {
        return false;
    };
    let Some(section) = parsed.section(0) else {
        return false;
    };
    if parsed.version() != 0x1122_3344
        || parsed.build() != 7
        || parsed.section_count() != 2
        || parsed.section(2).is_some()
        || parsed.paging_size() != Some(4096)
        || parsed.section_group(0) != Some(IntelFirmwareSectionGroup::Lmac)
        || parsed.section_group(1) != Some(IntelFirmwareSectionGroup::Umac)
        || parsed.section(1).map(|s| (s.device_offset, s.bytes))
            != Some((0x3000, &[5, 6, 7, 8][..]))
        || section.image != IntelFirmwareImageKind::Runtime
        || section.device_offset != 0x2000
        || section.bytes != [1, 2, 3, 4, 5, 6, 7, 8]
        || parsed.bytes().len() != cursor
    {
        return false;
    }

    let mut bad_magic = blob;
    bad_magic[4..8].copy_from_slice(&0u32.to_le_bytes());
    if !matches!(
        IntelTlvFirmware::parse(&bad_magic[..cursor]),
        Err(IntelTlvError::BadMagic)
    ) {
        return false;
    }

    let mut truncated = blob;
    truncated[INTEL_TLV_UCODE_HEADER_SIZE + 4..INTEL_TLV_UCODE_HEADER_SIZE + 8]
        .copy_from_slice(&0x1000u32.to_le_bytes());
    if !matches!(
        IntelTlvFirmware::parse(&truncated[..cursor]),
        Err(IntelTlvError::TruncatedTlv)
    ) {
        return false;
    }

    let mut too_short_section = [0u8; 100];
    too_short_section[4..8].copy_from_slice(&INTEL_TLV_UCODE_MAGIC.to_le_bytes());
    too_short_section[88..92].copy_from_slice(&INTEL_TLV_SEC_INIT.to_le_bytes());
    too_short_section[92..96].copy_from_slice(&3u32.to_le_bytes());
    if !matches!(
        IntelTlvFirmware::parse(&too_short_section),
        Err(IntelTlvError::SectionTooShort)
    ) {
        return false;
    }

    true
}
pub const INTEL_FIRMWARE_STAGE_CHUNK_SIZE: usize = crate::wifi_hw::DMA_PAGE_SIZE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareStageError {
    InvalidState,
    Separator,
    EmptySection,
    SectionTooLarge,
    AddressOverflow,
    Dma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareStageState {
    Ready,
    Staging,
    Staged,
    Published,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirmwareTransferDescriptor {
    pub device_offset: u32,
    pub physical_address: u64,
    pub length: u16,
}

pub struct IntelFirmwareStager<'a> {
    section: IntelFirmwareSection<'a>,
    state: FirmwareStageState,
    source_offset: usize,
    active: Option<crate::wifi_hw::OwnedDmaBuffer>,
    descriptor: Option<FirmwareTransferDescriptor>,
}

impl<'a> IntelFirmwareStager<'a> {
    pub fn new(section: IntelFirmwareSection<'a>) -> Result<Self, FirmwareStageError> {
        if section.is_separator() {
            return Err(FirmwareStageError::Separator);
        }
        if section.bytes.is_empty() {
            return Err(FirmwareStageError::EmptySection);
        }
        if section.bytes.len() > MAX_FIRMWARE_SECTION_SIZE {
            return Err(FirmwareStageError::SectionTooLarge);
        }
        let length =
            u32::try_from(section.bytes.len()).map_err(|_| FirmwareStageError::AddressOverflow)?;
        section
            .device_offset
            .checked_add(length)
            .ok_or(FirmwareStageError::AddressOverflow)?;

        Ok(Self {
            section,
            state: FirmwareStageState::Ready,
            source_offset: 0,
            active: None,
            descriptor: None,
        })
    }

    pub const fn state(&self) -> FirmwareStageState {
        self.state
    }

    pub const fn source_offset(&self) -> usize {
        self.source_offset
    }

    pub const fn is_complete(&self) -> bool {
        self.source_offset == self.section.bytes.len()
            && matches!(self.state, FirmwareStageState::Completed)
    }

    pub fn stage_next(&mut self) -> Result<FirmwareTransferDescriptor, FirmwareStageError> {
        if self.state != FirmwareStageState::Ready && self.state != FirmwareStageState::Completed {
            return Err(FirmwareStageError::InvalidState);
        }
        if self.source_offset >= self.section.bytes.len() {
            return Err(FirmwareStageError::InvalidState);
        }

        self.state = FirmwareStageState::Staging;
        let remaining = self.section.bytes.len() - self.source_offset;
        let chunk_len = remaining.min(INTEL_FIRMWARE_STAGE_CHUNK_SIZE);
        let length = u16::try_from(chunk_len).map_err(|_| FirmwareStageError::SectionTooLarge)?;
        let source_end = self
            .source_offset
            .checked_add(chunk_len)
            .ok_or(FirmwareStageError::AddressOverflow)?;
        let device_offset = self
            .section
            .device_offset
            .checked_add(
                u32::try_from(self.source_offset)
                    .map_err(|_| FirmwareStageError::AddressOverflow)?,
            )
            .ok_or(FirmwareStageError::AddressOverflow)?;

        let mut buffer = crate::wifi_hw::OwnedDmaBuffer::allocate(length, 0)
            .map_err(|_| FirmwareStageError::Dma)?;
        buffer
            .write_bytes(0, &self.section.bytes[self.source_offset..source_end])
            .map_err(|_| FirmwareStageError::Dma)?;

        let descriptor = FirmwareTransferDescriptor {
            device_offset,
            physical_address: buffer.physical_address(),
            length,
        };
        self.active = Some(buffer);
        self.descriptor = Some(descriptor);
        self.state = FirmwareStageState::Staged;
        Ok(descriptor)
    }

    /// Publish the staged chunk to the future chipset-specific transfer layer.
    ///
    /// Stage M models the ownership boundary but intentionally performs no
    /// Intel FH/PRPH register writes and rings no hardware doorbell.
    pub fn publish(&mut self) -> Result<FirmwareTransferDescriptor, FirmwareStageError> {
        if self.state != FirmwareStageState::Staged {
            return Err(FirmwareStageError::InvalidState);
        }
        let descriptor = self.descriptor.ok_or(FirmwareStageError::InvalidState)?;
        crate::wifi_hw::dma_publish();
        self.state = FirmwareStageState::Published;
        Ok(descriptor)
    }

    /// Complete a chunk only after the future hardware backend reports success.
    pub fn complete(&mut self) -> Result<(), FirmwareStageError> {
        if self.state != FirmwareStageState::Published {
            return Err(FirmwareStageError::InvalidState);
        }
        let descriptor = self.descriptor.ok_or(FirmwareStageError::InvalidState)?;
        crate::wifi_hw::dma_consume();
        self.source_offset = self
            .source_offset
            .checked_add(usize::from(descriptor.length))
            .ok_or(FirmwareStageError::AddressOverflow)?;
        self.active = None;
        self.descriptor = None;
        self.state = FirmwareStageState::Completed;
        Ok(())
    }

    pub fn staged_byte(&self, offset: usize) -> Result<u8, FirmwareStageError> {
        if self.state != FirmwareStageState::Staged && self.state != FirmwareStageState::Published {
            return Err(FirmwareStageError::InvalidState);
        }
        self.active
            .as_ref()
            .ok_or(FirmwareStageError::InvalidState)?
            .read_byte(offset)
            .map_err(|_| FirmwareStageError::Dma)
    }

    pub fn fail(&mut self) {
        self.active = None;
        self.descriptor = None;
        self.state = FirmwareStageState::Failed;
    }
}

pub fn stage13_10m_self_test() -> bool {
    // Separator records control the loader's image map; they are never DMA
    // payloads, even when the container includes four reserved payload bytes.
    for offset in [INTEL_CPU_SEPARATOR, INTEL_PAGING_SEPARATOR] {
        let separator = IntelFirmwareSection {
            image: IntelFirmwareImageKind::Runtime,
            device_offset: offset,
            bytes: &[0; 4],
        };
        if !matches!(
            IntelFirmwareStager::new(separator),
            Err(FirmwareStageError::Separator)
        ) {
            return false;
        }
    }
    let mut bytes = [0u8; 5000];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = (index & 0xff) as u8;
    }
    let section = IntelFirmwareSection {
        image: IntelFirmwareImageKind::Runtime,
        device_offset: 0x8000,
        bytes: &bytes,
    };
    let Ok(mut stager) = IntelFirmwareStager::new(section) else {
        return false;
    };

    if stager.state() != FirmwareStageState::Ready
        || stager.publish() != Err(FirmwareStageError::InvalidState)
    {
        return false;
    }

    let Ok(first) = stager.stage_next() else {
        return false;
    };
    if first.device_offset != 0x8000
        || usize::from(first.length) != INTEL_FIRMWARE_STAGE_CHUNK_SIZE
        || first.physical_address == 0
        || stager.state() != FirmwareStageState::Staged
        || stager.staged_byte(0) != Ok(bytes[0])
        || stager.staged_byte(4095) != Ok(bytes[4095])
        || stager.complete() != Err(FirmwareStageError::InvalidState)
    {
        return false;
    }
    if stager.publish() != Ok(first)
        || stager.state() != FirmwareStageState::Published
        || stager.complete().is_err()
        || stager.source_offset() != 4096
    {
        return false;
    }

    let Ok(second) = stager.stage_next() else {
        return false;
    };
    if second.device_offset != 0x9000
        || usize::from(second.length) != 904
        || stager.staged_byte(0) != Ok(bytes[4096])
        || stager.staged_byte(903) != Ok(bytes[4999])
        || stager.publish() != Ok(second)
        || stager.complete().is_err()
        || !stager.is_complete()
        || stager.stage_next() != Err(FirmwareStageError::InvalidState)
    {
        return false;
    }

    let overflow_section = IntelFirmwareSection {
        image: IntelFirmwareImageKind::Runtime,
        device_offset: 0xffff_fff0,
        bytes: &bytes[..32],
    };
    if !matches!(
        IntelFirmwareStager::new(overflow_section),
        Err(FirmwareStageError::AddressOverflow)
    ) {
        return false;
    }

    let small = [0x5au8; 32];
    let fail_section = IntelFirmwareSection {
        image: IntelFirmwareImageKind::Runtime,
        device_offset: 0x1000,
        bytes: &small,
    };
    let Ok(mut failed) = IntelFirmwareStager::new(fail_section) else {
        return false;
    };
    if failed.stage_next().is_err() {
        return false;
    }
    failed.fail();
    failed.state() == FirmwareStageState::Failed
        && failed.publish() == Err(FirmwareStageError::InvalidState)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareTransferExecutionState {
    Idle,
    Prepared,
    Started,
    Waiting,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareTransferExecutionError {
    InvalidState,
    InvalidDescriptor,
    BackendRejected,
    Timeout,
    HardwareError,
}

pub trait FirmwareTransferBackend {
    fn prepare(
        &mut self,
        descriptor: FirmwareTransferDescriptor,
    ) -> Result<(), FirmwareTransferExecutionError>;

    fn start(&mut self) -> Result<(), FirmwareTransferExecutionError>;

    fn completed(&mut self) -> Result<bool, FirmwareTransferExecutionError>;
}

pub struct IntelFirmwareTransferExecutor<B> {
    backend: B,
    state: FirmwareTransferExecutionState,
    descriptor: Option<FirmwareTransferDescriptor>,
}

impl<B: FirmwareTransferBackend> IntelFirmwareTransferExecutor<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            state: FirmwareTransferExecutionState::Idle,
            descriptor: None,
        }
    }

    pub const fn state(&self) -> FirmwareTransferExecutionState {
        self.state
    }

    pub fn prepare(
        &mut self,
        descriptor: FirmwareTransferDescriptor,
    ) -> Result<(), FirmwareTransferExecutionError> {
        if self.state != FirmwareTransferExecutionState::Idle {
            return Err(FirmwareTransferExecutionError::InvalidState);
        }

        if descriptor.physical_address == 0 || descriptor.length == 0 {
            self.state = FirmwareTransferExecutionState::Failed;
            return Err(FirmwareTransferExecutionError::InvalidDescriptor);
        }

        if let Err(error) = self.backend.prepare(descriptor) {
            self.state = FirmwareTransferExecutionState::Failed;
            return Err(error);
        }

        self.descriptor = Some(descriptor);
        self.state = FirmwareTransferExecutionState::Prepared;
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), FirmwareTransferExecutionError> {
        if self.state != FirmwareTransferExecutionState::Prepared {
            return Err(FirmwareTransferExecutionError::InvalidState);
        }

        if let Err(error) = self.backend.start() {
            self.state = FirmwareTransferExecutionState::Failed;
            return Err(error);
        }

        self.state = FirmwareTransferExecutionState::Started;
        Ok(())
    }

    pub fn wait_bounded(&mut self, limit: usize) -> Result<(), FirmwareTransferExecutionError> {
        if self.state != FirmwareTransferExecutionState::Started {
            return Err(FirmwareTransferExecutionError::InvalidState);
        }

        if limit == 0 {
            self.state = FirmwareTransferExecutionState::Failed;
            return Err(FirmwareTransferExecutionError::Timeout);
        }

        self.state = FirmwareTransferExecutionState::Waiting;

        for _ in 0..limit {
            match self.backend.completed() {
                Ok(true) => {
                    self.state = FirmwareTransferExecutionState::Completed;
                    return Ok(());
                }
                Ok(false) => {}
                Err(error) => {
                    self.state = FirmwareTransferExecutionState::Failed;
                    return Err(error);
                }
            }
        }

        self.state = FirmwareTransferExecutionState::Failed;
        Err(FirmwareTransferExecutionError::Timeout)
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

#[derive(Clone, Copy)]
struct Stage13_10nSyntheticBackend {
    descriptor: Option<FirmwareTransferDescriptor>,
    started: bool,
    polls_before_completion: usize,
    polls: usize,
    fail_start: bool,
}

impl Stage13_10nSyntheticBackend {
    const fn new(polls_before_completion: usize) -> Self {
        Self {
            descriptor: None,
            started: false,
            polls_before_completion,
            polls: 0,
            fail_start: false,
        }
    }
}

impl FirmwareTransferBackend for Stage13_10nSyntheticBackend {
    fn prepare(
        &mut self,
        descriptor: FirmwareTransferDescriptor,
    ) -> Result<(), FirmwareTransferExecutionError> {
        self.descriptor = Some(descriptor);
        Ok(())
    }

    fn start(&mut self) -> Result<(), FirmwareTransferExecutionError> {
        if self.descriptor.is_none() || self.fail_start {
            return Err(FirmwareTransferExecutionError::BackendRejected);
        }

        self.started = true;
        Ok(())
    }

    fn completed(&mut self) -> Result<bool, FirmwareTransferExecutionError> {
        if !self.started {
            return Err(FirmwareTransferExecutionError::InvalidState);
        }

        self.polls = self.polls.saturating_add(1);
        Ok(self.polls >= self.polls_before_completion)
    }
}

pub fn stage13_10n_self_test() -> bool {
    let bytes = [0x5au8; 128];
    let section = IntelFirmwareSection {
        image: IntelFirmwareImageKind::Runtime,
        device_offset: 0x8000,
        bytes: &bytes,
    };

    let Ok(mut stager) = IntelFirmwareStager::new(section) else {
        return false;
    };
    let Ok(descriptor) = stager.stage_next() else {
        return false;
    };
    if stager.publish() != Ok(descriptor) {
        return false;
    }

    let backend = Stage13_10nSyntheticBackend::new(3);
    let mut executor = IntelFirmwareTransferExecutor::new(backend);

    if executor.start() != Err(FirmwareTransferExecutionError::InvalidState)
        || executor.prepare(descriptor).is_err()
        || executor.state() != FirmwareTransferExecutionState::Prepared
        || executor.start().is_err()
        || executor.state() != FirmwareTransferExecutionState::Started
        || executor.wait_bounded(8).is_err()
        || executor.state() != FirmwareTransferExecutionState::Completed
    {
        return false;
    }

    if stager.complete().is_err() || !stager.is_complete() {
        return false;
    }

    let timeout_descriptor = FirmwareTransferDescriptor {
        device_offset: 0x9000,
        physical_address: 0x1000,
        length: 64,
    };
    let timeout_backend = Stage13_10nSyntheticBackend::new(usize::MAX);
    let mut timeout_executor = IntelFirmwareTransferExecutor::new(timeout_backend);

    if timeout_executor.prepare(timeout_descriptor).is_err()
        || timeout_executor.start().is_err()
        || timeout_executor.wait_bounded(4) != Err(FirmwareTransferExecutionError::Timeout)
        || timeout_executor.state() != FirmwareTransferExecutionState::Failed
    {
        return false;
    }

    let invalid_descriptor = FirmwareTransferDescriptor {
        device_offset: 0x1000,
        physical_address: 0,
        length: 32,
    };
    let invalid_backend = Stage13_10nSyntheticBackend::new(1);
    let mut invalid_executor = IntelFirmwareTransferExecutor::new(invalid_backend);

    invalid_executor.prepare(invalid_descriptor)
        == Err(FirmwareTransferExecutionError::InvalidDescriptor)
        && invalid_executor.state() == FirmwareTransferExecutionState::Failed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intel22000TransferStatus {
    Idle,
    Busy,
    Complete,
    Error,
}

/// Narrow hardware contract for Intel 22000-family firmware DMA transport.
///
/// This is intentionally below `IntelFirmwareTransferExecutor`: the executor
/// owns generic prepare/start/wait policy, while this trait owns the eventual
/// generation-specific register programming.
pub trait Intel22000TransferIo {
    fn program_source(
        &mut self,
        physical_address: u64,
    ) -> Result<(), FirmwareTransferExecutionError>;
    fn program_destination(
        &mut self,
        device_offset: u32,
    ) -> Result<(), FirmwareTransferExecutionError>;
    fn program_length(&mut self, length: u16) -> Result<(), FirmwareTransferExecutionError>;
    fn trigger(&mut self) -> Result<(), FirmwareTransferExecutionError>;
    fn status(&mut self) -> Result<Intel22000TransferStatus, FirmwareTransferExecutionError>;
}

pub struct Intel22000FirmwareTransferBackend<I> {
    io: I,
    prepared: bool,
    started: bool,
}

impl<I: Intel22000TransferIo> Intel22000FirmwareTransferBackend<I> {
    pub const fn new(io: I) -> Self {
        Self {
            io,
            prepared: false,
            started: false,
        }
    }

    pub fn into_io(self) -> I {
        self.io
    }
}

impl<I: Intel22000TransferIo> FirmwareTransferBackend for Intel22000FirmwareTransferBackend<I> {
    fn prepare(
        &mut self,
        descriptor: FirmwareTransferDescriptor,
    ) -> Result<(), FirmwareTransferExecutionError> {
        if self.prepared || self.started {
            return Err(FirmwareTransferExecutionError::InvalidState);
        }
        if descriptor.physical_address == 0 || descriptor.length == 0 {
            return Err(FirmwareTransferExecutionError::InvalidDescriptor);
        }

        self.io.program_source(descriptor.physical_address)?;
        self.io.program_destination(descriptor.device_offset)?;
        self.io.program_length(descriptor.length)?;
        self.prepared = true;
        Ok(())
    }

    fn start(&mut self) -> Result<(), FirmwareTransferExecutionError> {
        if !self.prepared || self.started {
            return Err(FirmwareTransferExecutionError::InvalidState);
        }
        self.io.trigger()?;
        self.started = true;
        Ok(())
    }

    fn completed(&mut self) -> Result<bool, FirmwareTransferExecutionError> {
        if !self.started {
            return Err(FirmwareTransferExecutionError::InvalidState);
        }

        match self.io.status()? {
            Intel22000TransferStatus::Complete => Ok(true),
            Intel22000TransferStatus::Busy => Ok(false),
            Intel22000TransferStatus::Idle => Err(FirmwareTransferExecutionError::HardwareError),
            Intel22000TransferStatus::Error => Err(FirmwareTransferExecutionError::HardwareError),
        }
    }
}

#[derive(Clone, Copy)]
struct Stage13_10oSyntheticIntel22000Io {
    source: u64,
    destination: u32,
    length: u16,
    triggered: bool,
    polls: usize,
    complete_after: usize,
    force_error: bool,
}

impl Stage13_10oSyntheticIntel22000Io {
    const fn new(complete_after: usize) -> Self {
        Self {
            source: 0,
            destination: 0,
            length: 0,
            triggered: false,
            polls: 0,
            complete_after,
            force_error: false,
        }
    }

    const fn programmed(&self) -> bool {
        self.source != 0 && self.length != 0
    }
}

impl Intel22000TransferIo for Stage13_10oSyntheticIntel22000Io {
    fn program_source(
        &mut self,
        physical_address: u64,
    ) -> Result<(), FirmwareTransferExecutionError> {
        if physical_address == 0 {
            return Err(FirmwareTransferExecutionError::InvalidDescriptor);
        }
        self.source = physical_address;
        Ok(())
    }

    fn program_destination(
        &mut self,
        device_offset: u32,
    ) -> Result<(), FirmwareTransferExecutionError> {
        self.destination = device_offset;
        Ok(())
    }

    fn program_length(&mut self, length: u16) -> Result<(), FirmwareTransferExecutionError> {
        if length == 0 {
            return Err(FirmwareTransferExecutionError::InvalidDescriptor);
        }
        self.length = length;
        Ok(())
    }

    fn trigger(&mut self) -> Result<(), FirmwareTransferExecutionError> {
        if !self.programmed() {
            return Err(FirmwareTransferExecutionError::BackendRejected);
        }
        self.triggered = true;
        Ok(())
    }

    fn status(&mut self) -> Result<Intel22000TransferStatus, FirmwareTransferExecutionError> {
        if self.force_error {
            return Ok(Intel22000TransferStatus::Error);
        }
        if !self.triggered {
            return Ok(Intel22000TransferStatus::Idle);
        }
        self.polls = self.polls.saturating_add(1);
        if self.polls >= self.complete_after {
            Ok(Intel22000TransferStatus::Complete)
        } else {
            Ok(Intel22000TransferStatus::Busy)
        }
    }
}

pub fn stage13_10o_self_test() -> bool {
    let descriptor = FirmwareTransferDescriptor {
        device_offset: 0x8000,
        physical_address: 0x0020_0000,
        length: 4096,
    };

    let io = Stage13_10oSyntheticIntel22000Io::new(3);
    let backend = Intel22000FirmwareTransferBackend::new(io);
    let mut executor = IntelFirmwareTransferExecutor::new(backend);

    if executor.prepare(descriptor).is_err()
        || executor.start().is_err()
        || executor.wait_bounded(8).is_err()
        || executor.state() != FirmwareTransferExecutionState::Completed
    {
        return false;
    }

    let backend = executor.into_backend();
    let io = backend.into_io();
    if io.source != descriptor.physical_address
        || io.destination != descriptor.device_offset
        || io.length != descriptor.length
        || !io.triggered
        || io.polls != 3
    {
        return false;
    }

    // A 22000-family transport error must propagate through the generic
    // executor and leave the transfer failed rather than reclaiming DMA.
    let mut error_io = Stage13_10oSyntheticIntel22000Io::new(1);
    error_io.force_error = true;
    let error_backend = Intel22000FirmwareTransferBackend::new(error_io);
    let mut error_executor = IntelFirmwareTransferExecutor::new(error_backend);

    error_executor.prepare(descriptor).is_ok()
        && error_executor.start().is_ok()
        && error_executor.wait_bounded(2) == Err(FirmwareTransferExecutionError::HardwareError)
        && error_executor.state() == FirmwareTransferExecutionState::Failed
}

pub const INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES: usize = ax200_image::AX200_MAX_IMAGE_BUFFERS;
pub const INTEL_CONTEXT_INFO_MAX_DRAM_CHUNK_SIZE: usize = ax200_image::AX200_MAX_IMAGE_CHUNK;
pub const INTEL_CSR_CONTEXT_INFO_BASE: u32 = 0x40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelContextInfoError {
    ZeroPhysicalAddress,
    EmptySection,
    SectionTooLarge,
    TooManySections,
    MissingQueueAddress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntelContextInfoDramEntry {
    pub physical_address: u64,
    pub length: u32,
}

impl IntelContextInfoDramEntry {
    pub const EMPTY: Self = Self {
        physical_address: 0,
        length: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelContextInfoImageKind {
    Lmac,
    Umac,
    Paging,
}

/// AX200/22000-generation context-info DRAM map.
///
/// Linux iwlwifi uses a context-info self-load path for device family 22000:
/// firmware sections are copied to coherent host DRAM and the context-info
/// structure publishes their physical addresses to the device.  This model
/// deliberately keeps the host-side manifest separate from the MMIO kick.
pub struct Intel22000ContextInfoManifest {
    lmac: [IntelContextInfoDramEntry; INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES],
    umac: [IntelContextInfoDramEntry; INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES],
    paging: [IntelContextInfoDramEntry; INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES],
    lmac_count: usize,
    umac_count: usize,
    paging_count: usize,
    free_rbd_address: u64,
    used_rbd_address: u64,
    status_write_pointer: u64,
    command_queue_address: u64,
    command_queue_size: u8,
}

impl Intel22000ContextInfoManifest {
    pub const fn new() -> Self {
        Self {
            lmac: [IntelContextInfoDramEntry::EMPTY; INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES],
            umac: [IntelContextInfoDramEntry::EMPTY; INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES],
            paging: [IntelContextInfoDramEntry::EMPTY; INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES],
            lmac_count: 0,
            umac_count: 0,
            paging_count: 0,
            free_rbd_address: 0,
            used_rbd_address: 0,
            status_write_pointer: 0,
            command_queue_address: 0,
            command_queue_size: 0,
        }
    }

    pub fn set_rx_queue(
        &mut self,
        free_rbd_address: u64,
        used_rbd_address: u64,
        status_write_pointer: u64,
    ) -> Result<(), IntelContextInfoError> {
        if free_rbd_address == 0 || used_rbd_address == 0 || status_write_pointer == 0 {
            return Err(IntelContextInfoError::MissingQueueAddress);
        }
        self.free_rbd_address = free_rbd_address;
        self.used_rbd_address = used_rbd_address;
        self.status_write_pointer = status_write_pointer;
        Ok(())
    }

    pub fn set_command_queue(
        &mut self,
        address: u64,
        size: u8,
    ) -> Result<(), IntelContextInfoError> {
        if address == 0 || size == 0 {
            return Err(IntelContextInfoError::MissingQueueAddress);
        }
        self.command_queue_address = address;
        self.command_queue_size = size;
        Ok(())
    }

    pub fn push(
        &mut self,
        kind: IntelContextInfoImageKind,
        entry: IntelContextInfoDramEntry,
    ) -> Result<(), IntelContextInfoError> {
        if entry.physical_address == 0 {
            return Err(IntelContextInfoError::ZeroPhysicalAddress);
        }
        if entry.length == 0 {
            return Err(IntelContextInfoError::EmptySection);
        }
        if usize::try_from(entry.length).map_err(|_| IntelContextInfoError::SectionTooLarge)?
            > INTEL_CONTEXT_INFO_MAX_DRAM_CHUNK_SIZE
        {
            return Err(IntelContextInfoError::SectionTooLarge);
        }

        let (entries, count) = match kind {
            IntelContextInfoImageKind::Lmac => (&mut self.lmac, &mut self.lmac_count),
            IntelContextInfoImageKind::Umac => (&mut self.umac, &mut self.umac_count),
            IntelContextInfoImageKind::Paging => (&mut self.paging, &mut self.paging_count),
        };

        if *count >= entries.len() {
            return Err(IntelContextInfoError::TooManySections);
        }
        entries[*count] = entry;
        *count += 1;
        Ok(())
    }

    pub const fn lmac_count(&self) -> usize {
        self.lmac_count
    }

    pub const fn umac_count(&self) -> usize {
        self.umac_count
    }

    pub const fn paging_count(&self) -> usize {
        self.paging_count
    }

    pub const fn queues_ready(&self) -> bool {
        self.free_rbd_address != 0
            && self.used_rbd_address != 0
            && self.status_write_pointer != 0
            && self.command_queue_address != 0
            && self.command_queue_size != 0
    }

    pub fn entry(
        &self,
        kind: IntelContextInfoImageKind,
        index: usize,
    ) -> Option<IntelContextInfoDramEntry> {
        let (entries, count) = match kind {
            IntelContextInfoImageKind::Lmac => (&self.lmac, self.lmac_count),
            IntelContextInfoImageKind::Umac => (&self.umac, self.umac_count),
            IntelContextInfoImageKind::Paging => (&self.paging, self.paging_count),
        };
        if index >= count {
            None
        } else {
            Some(entries[index])
        }
    }
}

pub fn stage13_10p_self_test() -> bool {
    let mut manifest = Intel22000ContextInfoManifest::new();

    if manifest.queues_ready()
        || manifest
            .set_rx_queue(0x0010_0000, 0x0011_0000, 0x0012_0000)
            .is_err()
        || manifest
            .set_command_queue(0x0013_0000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || !manifest.queues_ready()
    {
        return false;
    }

    let lmac = IntelContextInfoDramEntry {
        physical_address: 0x0020_0000,
        length: 4096,
    };
    let umac = IntelContextInfoDramEntry {
        physical_address: 0x0021_0000,
        length: 8192,
    };
    let paging = IntelContextInfoDramEntry {
        physical_address: 0x0022_0000,
        length: 32 * 1024,
    };

    if manifest
        .push(IntelContextInfoImageKind::Lmac, lmac)
        .is_err()
        || manifest
            .push(IntelContextInfoImageKind::Umac, umac)
            .is_err()
        || manifest
            .push(IntelContextInfoImageKind::Paging, paging)
            .is_err()
        || manifest.lmac_count() != 1
        || manifest.umac_count() != 1
        || manifest.paging_count() != 1
        || manifest.entry(IntelContextInfoImageKind::Lmac, 0) != Some(lmac)
        || manifest.entry(IntelContextInfoImageKind::Umac, 0) != Some(umac)
        || manifest.entry(IntelContextInfoImageKind::Paging, 0) != Some(paging)
    {
        return false;
    }

    let too_large = IntelContextInfoDramEntry {
        physical_address: 0x0030_0000,
        length: (INTEL_CONTEXT_INFO_MAX_DRAM_CHUNK_SIZE as u32) + 1,
    };
    let zero_address = IntelContextInfoDramEntry {
        physical_address: 0,
        length: 4096,
    };

    manifest.push(IntelContextInfoImageKind::Lmac, too_large)
        == Err(IntelContextInfoError::SectionTooLarge)
        && manifest.push(IntelContextInfoImageKind::Lmac, zero_address)
            == Err(IntelContextInfoError::ZeroPhysicalAddress)
        && INTEL_CSR_CONTEXT_INFO_BASE == 0x40
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelContextInfoBuildError {
    Image(Ax200ImageError),
    Manifest(IntelContextInfoError),
    Dma,
    LengthOverflow,
}

pub struct Intel22000DmaContextInfo {
    manifest: Intel22000ContextInfoManifest,
    firmware_dma: [Option<crate::wifi_hw::OwnedDmaBuffer>; INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES],
    firmware_dma_count: usize,
}

impl Intel22000DmaContextInfo {
    /// Validate the entire runtime layout before allocating, then copy each
    /// payload into owned DMA memory. An error drops all unpublished buffers.
    /// This neither configures queues nor publishes addresses to hardware.
    pub fn from_runtime_firmware(
        firmware: &IntelTlvFirmware<'_>,
    ) -> Result<Self, IntelContextInfoBuildError> {
        let image =
            Ax200RuntimeImage::validate(firmware).map_err(IntelContextInfoBuildError::Image)?;
        let counts = image.counts();
        let mut context = Self::new();
        for (region, bytes) in image.payloads() {
            let kind = match region {
                Ax200ImageRegion::Lmac => IntelContextInfoImageKind::Lmac,
                Ax200ImageRegion::Umac => IntelContextInfoImageKind::Umac,
                Ax200ImageRegion::Paging => IntelContextInfoImageKind::Paging,
            };
            context.stage_firmware_chunk(kind, bytes)?;
        }
        debug_assert_eq!(context.firmware_buffer_count(), counts.iter().sum());
        Ok(context)
    }

    pub fn new() -> Self {
        Self {
            manifest: Intel22000ContextInfoManifest::new(),
            firmware_dma: [const { None }; INTEL_CONTEXT_INFO_MAX_DRAM_ENTRIES],
            firmware_dma_count: 0,
        }
    }
    pub fn manifest(&self) -> &Intel22000ContextInfoManifest {
        &self.manifest
    }
    pub fn set_rx_queue(
        &mut self,
        free: u64,
        used: u64,
        status: u64,
    ) -> Result<(), IntelContextInfoBuildError> {
        self.manifest
            .set_rx_queue(free, used, status)
            .map_err(IntelContextInfoBuildError::Manifest)
    }
    pub fn set_command_queue(
        &mut self,
        address: u64,
        size: u8,
    ) -> Result<(), IntelContextInfoBuildError> {
        self.manifest
            .set_command_queue(address, size)
            .map_err(IntelContextInfoBuildError::Manifest)
    }
    pub fn stage_firmware_chunk(
        &mut self,
        kind: IntelContextInfoImageKind,
        bytes: &[u8],
    ) -> Result<IntelContextInfoDramEntry, IntelContextInfoBuildError> {
        if bytes.is_empty() {
            return Err(IntelContextInfoBuildError::Manifest(
                IntelContextInfoError::EmptySection,
            ));
        }
        if bytes.len() > INTEL_CONTEXT_INFO_MAX_DRAM_CHUNK_SIZE {
            return Err(IntelContextInfoBuildError::Manifest(
                IntelContextInfoError::SectionTooLarge,
            ));
        }
        let length =
            u16::try_from(bytes.len()).map_err(|_| IntelContextInfoBuildError::LengthOverflow)?;
        let manifest_length =
            u32::try_from(bytes.len()).map_err(|_| IntelContextInfoBuildError::LengthOverflow)?;
        let mut buffer = crate::wifi_hw::OwnedDmaBuffer::allocate(length, 0)
            .map_err(|_| IntelContextInfoBuildError::Dma)?;
        buffer
            .write_bytes(0, bytes)
            .map_err(|_| IntelContextInfoBuildError::Dma)?;
        let entry = IntelContextInfoDramEntry {
            physical_address: buffer.physical_address(),
            length: manifest_length,
        };
        if self.firmware_dma_count >= self.firmware_dma.len() {
            return Err(IntelContextInfoBuildError::Manifest(
                IntelContextInfoError::TooManySections,
            ));
        }
        self.manifest
            .push(kind, entry)
            .map_err(IntelContextInfoBuildError::Manifest)?;
        self.firmware_dma[self.firmware_dma_count] = Some(buffer);
        self.firmware_dma_count += 1;
        Ok(entry)
    }
    pub fn firmware_buffer_count(&self) -> usize {
        self.firmware_dma_count
    }
    pub fn staged_byte(
        &self,
        buffer_index: usize,
        offset: usize,
    ) -> Result<u8, IntelContextInfoBuildError> {
        self.firmware_dma
            .get(buffer_index)
            .and_then(Option::as_ref)
            .ok_or(IntelContextInfoBuildError::Dma)?
            .read_byte(offset)
            .map_err(|_| IntelContextInfoBuildError::Dma)
    }
    pub fn ready_for_publication(&self) -> bool {
        self.manifest.queues_ready() && self.firmware_dma_count != 0
    }
}

// === Stage 13.10R: Intel 22000/AX200 context-info hardware ABI ===
pub const INTEL_CONTEXT_INFO_WIRE_SIZE: usize = 1792;
pub const INTEL_CONTEXT_INFO_WIRE_DWORDS: u16 = 448;
pub const INTEL_CONTEXT_INFO_DRAM_UMAC_OFFSET: usize = 192;
pub const INTEL_CONTEXT_INFO_DRAM_LMAC_OFFSET: usize = 704;
pub const INTEL_CONTEXT_INFO_DRAM_PAGING_OFFSET: usize = 1216;
pub const INTEL_CONTEXT_INFO_DEFAULT_CONTROL_FLAGS: u32 = 0x0100 | (9 << 4) | (4 << 9);
/// Intel's context-info hcmd_cfg stores TFD_QUEUE_CB_SIZE(entry_count), not
/// the number of descriptors.  The upstream formula is ilog2(x) - 3, so the
/// 32-entry AX201 command queue encodes as 2.
pub const INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelContextInfoAbiError {
    NotReady,
    Dma,
    TooManySections,
    Publication,
}
pub struct Intel22000HardwareContextInfo {
    dma: crate::wifi_hw::OwnedDmaBuffer,
}
impl Intel22000HardwareContextInfo {
    pub const fn physical_address(&self) -> u64 {
        self.dma.physical_address()
    }
    pub fn byte(&self, o: usize) -> Result<u8, IntelContextInfoAbiError> {
        self.dma
            .read_byte(o)
            .map_err(|_| IntelContextInfoAbiError::Dma)
    }
}
fn ci16(x: &mut [u8; INTEL_CONTEXT_INFO_WIRE_SIZE], o: usize, v: u16) {
    x[o..o + 2].copy_from_slice(&v.to_le_bytes());
}
fn ci32(x: &mut [u8; INTEL_CONTEXT_INFO_WIRE_SIZE], o: usize, v: u32) {
    x[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn ci64(x: &mut [u8; INTEL_CONTEXT_INFO_WIRE_SIZE], o: usize, v: u64) {
    x[o..o + 8].copy_from_slice(&v.to_le_bytes());
}
impl Intel22000DmaContextInfo {
    pub fn build_hardware_context(
        &self,
        mac_id: u16,
    ) -> Result<Intel22000HardwareContextInfo, IntelContextInfoAbiError> {
        if !self.ready_for_publication() {
            return Err(IntelContextInfoAbiError::NotReady);
        }
        let mut x = [0u8; INTEL_CONTEXT_INFO_WIRE_SIZE];
        ci16(&mut x, 0, mac_id);
        ci16(&mut x, 4, INTEL_CONTEXT_INFO_WIRE_DWORDS);
        ci32(&mut x, 8, INTEL_CONTEXT_INFO_DEFAULT_CONTROL_FLAGS);
        ci64(&mut x, 24, self.manifest.free_rbd_address);
        ci64(&mut x, 32, self.manifest.used_rbd_address);
        ci64(&mut x, 40, self.manifest.status_write_pointer);
        ci64(&mut x, 48, self.manifest.command_queue_address);
        x[56] = self.manifest.command_queue_size;
        for (kind, base, count) in [
            (
                IntelContextInfoImageKind::Umac,
                INTEL_CONTEXT_INFO_DRAM_UMAC_OFFSET,
                self.manifest.umac_count(),
            ),
            (
                IntelContextInfoImageKind::Lmac,
                INTEL_CONTEXT_INFO_DRAM_LMAC_OFFSET,
                self.manifest.lmac_count(),
            ),
            (
                IntelContextInfoImageKind::Paging,
                INTEL_CONTEXT_INFO_DRAM_PAGING_OFFSET,
                self.manifest.paging_count(),
            ),
        ] {
            if count > 64 {
                return Err(IntelContextInfoAbiError::TooManySections);
            }
            for i in 0..count {
                let e = self
                    .manifest
                    .entry(kind, i)
                    .ok_or(IntelContextInfoAbiError::TooManySections)?;
                ci64(&mut x, base + i * 8, e.physical_address);
            }
        }
        let mut dma =
            crate::wifi_hw::OwnedDmaBuffer::allocate(INTEL_CONTEXT_INFO_WIRE_SIZE as u16, 0)
                .map_err(|_| IntelContextInfoAbiError::Dma)?;
        dma.write_bytes(0, &x)
            .map_err(|_| IntelContextInfoAbiError::Dma)?;
        Ok(Intel22000HardwareContextInfo { dma })
    }
}
pub trait IntelContextInfoPublicationIo {
    fn write_context_info_base(&mut self, o: u32, a: u64) -> Result<(), IntelContextInfoAbiError>;
}
pub fn publish_hardware_context<I: IntelContextInfoPublicationIo>(
    c: &Intel22000HardwareContextInfo,
    io: &mut I,
) -> Result<(), IntelContextInfoAbiError> {
    crate::wifi_hw::dma_publish();
    io.write_context_info_base(INTEL_CSR_CONTEXT_INFO_BASE, c.physical_address())
        .map_err(|_| IntelContextInfoAbiError::Publication)
}
struct Stage13_10rMockIo {
    offset: u32,
    address: u64,
    writes: usize,
}
impl IntelContextInfoPublicationIo for Stage13_10rMockIo {
    fn write_context_info_base(&mut self, o: u32, a: u64) -> Result<(), IntelContextInfoAbiError> {
        self.offset = o;
        self.address = a;
        self.writes += 1;
        Ok(())
    }
}
// Stage 13.10S
pub const INTEL_UREG_CPU_INIT_RUN: u32 = 0x00a0_5c44;
pub const INTEL_CPU_INIT_RUN_VALUE: u32 = 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelFirmwareStartState {
    Reset,
    ContextPublished,
    CpuRunIssued,
    AwaitingAlive,
    FirmwareRunning,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelFirmwareStartError {
    InvalidState,
    Publication(IntelContextInfoAbiError),
    Prph,
}
pub trait IntelFirmwareStartupIo: IntelContextInfoPublicationIo {
    fn write_prph(&mut self, r: u32, v: u32) -> Result<(), IntelFirmwareStartError>;
    fn program_rx_queue(
        &mut self,
        free_rbd: u64,
        used_rbd: u64,
        status: u64,
    ) -> Result<(), IntelFirmwareStartError> {
        let _ = (free_rbd, used_rbd, status);
        Ok(())
    }
    fn ring_command_queue(
        &mut self,
        queue_id: u8,
        write_pointer: u16,
    ) -> Result<(), IntelFirmwareStartError> {
        let _ = (queue_id, write_pointer);
        Err(IntelFirmwareStartError::Prph)
    }
    fn ring_rx_queue(&mut self, write_index: u16) -> Result<(), IntelFirmwareStartError> {
        let _ = write_index;
        Ok(())
    }
}

impl IntelContextInfoPublicationIo for crate::wifi_hw::Intel22000MmioIo {
    fn write_context_info_base(
        &mut self,
        offset: u32,
        address: u64,
    ) -> Result<(), IntelContextInfoAbiError> {
        self.write_context_info_base(offset, address)
            .map_err(|_| IntelContextInfoAbiError::Publication)
    }
}

impl IntelFirmwareStartupIo for crate::wifi_hw::Intel22000MmioIo {
    fn write_prph(&mut self, register: u32, value: u32) -> Result<(), IntelFirmwareStartError> {
        self.write_prph(register, value)
            .map_err(|_| IntelFirmwareStartError::Prph)
    }

    fn program_rx_queue(
        &mut self,
        free_rbd: u64,
        used_rbd: u64,
        status: u64,
    ) -> Result<(), IntelFirmwareStartError> {
        self.program_rx_queue(free_rbd, used_rbd, status)
            .map_err(|_| IntelFirmwareStartError::Prph)
    }

    fn ring_command_queue(
        &mut self,
        queue_id: u8,
        write_pointer: u16,
    ) -> Result<(), IntelFirmwareStartError> {
        self.ring_command_queue(queue_id, write_pointer)
            .map_err(|_| IntelFirmwareStartError::Prph)
    }

    fn ring_rx_queue(&mut self, write_index: u16) -> Result<(), IntelFirmwareStartError> {
        self.ring_rx_queue(write_index)
            .map_err(|_| IntelFirmwareStartError::Prph)
    }
}

pub struct Intel22000FirmwareStartup<I> {
    io: I,
    state: IntelFirmwareStartState,
}
impl<I: IntelFirmwareStartupIo> Intel22000FirmwareStartup<I> {
    pub fn new(io: I) -> Self {
        Self {
            io,
            state: IntelFirmwareStartState::Reset,
        }
    }
    pub const fn state(&self) -> IntelFirmwareStartState {
        self.state
    }
    pub fn publish_context(
        &mut self,
        c: &Intel22000HardwareContextInfo,
    ) -> Result<(), IntelFirmwareStartError> {
        if self.state != IntelFirmwareStartState::Reset {
            return Err(IntelFirmwareStartError::InvalidState);
        }
        publish_hardware_context(c, &mut self.io).map_err(IntelFirmwareStartError::Publication)?;
        self.state = IntelFirmwareStartState::ContextPublished;
        Ok(())
    }
    pub fn issue_cpu_init_run(&mut self) -> Result<(), IntelFirmwareStartError> {
        if self.state != IntelFirmwareStartState::ContextPublished {
            return Err(IntelFirmwareStartError::InvalidState);
        }
        if self
            .io
            .write_prph(INTEL_UREG_CPU_INIT_RUN, INTEL_CPU_INIT_RUN_VALUE)
            .is_err()
        {
            self.state = IntelFirmwareStartState::Failed;
            return Err(IntelFirmwareStartError::Prph);
        }
        self.state = IntelFirmwareStartState::CpuRunIssued;
        Ok(())
    }
    pub fn program_rx_queue(
        &mut self,
        free_rbd: u64,
        used_rbd: u64,
        status: u64,
    ) -> Result<(), IntelFirmwareStartError> {
        if self.state != IntelFirmwareStartState::ContextPublished {
            return Err(IntelFirmwareStartError::InvalidState);
        }
        self.io.program_rx_queue(free_rbd, used_rbd, status)
    }
    pub fn ring_rx_queue(&mut self, write_index: u16) -> Result<(), IntelFirmwareStartError> {
        if self.state != IntelFirmwareStartState::ContextPublished {
            return Err(IntelFirmwareStartError::InvalidState);
        }
        self.io.ring_rx_queue(write_index)
    }
    pub fn begin_alive_wait(&mut self) -> Result<(), IntelFirmwareStartError> {
        if self.state != IntelFirmwareStartState::CpuRunIssued {
            return Err(IntelFirmwareStartError::InvalidState);
        }
        self.state = IntelFirmwareStartState::AwaitingAlive;
        Ok(())
    }
    pub fn ring_command_queue(
        &mut self,
        queue_id: u8,
        write_pointer: u16,
    ) -> Result<(), IntelFirmwareStartError> {
        if self.state != IntelFirmwareStartState::FirmwareRunning {
            return Err(IntelFirmwareStartError::InvalidState);
        }
        self.io.ring_command_queue(queue_id, write_pointer)
    }
    pub fn into_inner(self) -> I {
        self.io
    }
}

// === Stage 13.10AG: AX201 driver ownership/lifecycle ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201DriverState {
    Prepared,
    AwaitingAlive,
    Running,
    Failed,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201CommandStatusError {
    Notification(IntelRxNotificationError),
    MissingStatus,
    Firmware(u32),
    Unsolicited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201DriverError {
    UnsupportedFunction,
    Firmware(Intel22000FirmwarePrepareError),
    Queues(crate::wifi_hw::Ax201QueueError),
    Queue(IntelContextInfoBuildError),
    Context(IntelContextInfoAbiError),
    Startup(IntelFirmwareStartError),
    NotStarted,
    QueueDma,
    Command(crate::wifi_hw::Ax201QueueError),
    CommandResponse(IntelRxNotificationError),
    CommandStatus(Ax201CommandStatusError),
    PhyContext(Ax201PhyContextError),
    MacContext(Ax201MacContextError),
    BindingContext(Ax201BindingContextError),
    Scan(Ax201ScanError),
    Tx(Ax201TxError),
}

/// Owns the AX201 runtime image, context-info DMA and startup state as one
/// teardown unit. No borrowed firmware bytes survive construction; the only
/// borrowed input is consumed while `prepare_ax201_runtime_firmware` copies
/// it into kernel-owned DMA pages.
pub struct Ax201Driver<I> {
    function: crate::wifi_hw::SupportedWifiFunction,
    context: Intel22000DmaContextInfo,
    queues: Option<crate::wifi_hw::Ax201QueueOwner>,
    hardware_context: Option<Intel22000HardwareContextInfo>,
    startup: Option<Intel22000FirmwareStartup<I>>,
    scan_uid: Option<u32>,
    tx_queue_id: Option<u8>,
    state: Ax201DriverState,
}

impl<I: IntelFirmwareStartupIo> Ax201Driver<I> {
    pub fn new(
        function: crate::wifi_hw::SupportedWifiFunction,
        firmware_bytes: &[u8],
        io: I,
    ) -> Result<Self, Ax201DriverError> {
        if function.family != crate::wifi_hw::WifiDriverFamily::IntelIwlwifiAx201 {
            return Err(Ax201DriverError::UnsupportedFunction);
        }
        let context = prepare_ax201_runtime_firmware(function, firmware_bytes)
            .map_err(Ax201DriverError::Firmware)?;
        Ok(Self {
            function,
            context,
            queues: None,
            hardware_context: None,
            startup: Some(Intel22000FirmwareStartup::new(io)),
            scan_uid: None,
            tx_queue_id: None,
            state: Ax201DriverState::Prepared,
        })
    }

    pub fn new_from_source(
        function: crate::wifi_hw::SupportedWifiFunction,
        source: Ax201FirmwareSource<'_>,
        io: I,
    ) -> Result<Self, Ax201DriverError> {
        if function.family != crate::wifi_hw::WifiDriverFamily::IntelIwlwifiAx201 {
            return Err(Ax201DriverError::UnsupportedFunction);
        }
        let context = prepare_ax201_runtime_firmware_from_source(function, source)
            .map_err(Ax201DriverError::Firmware)?;
        Ok(Self {
            function,
            context,
            queues: None,
            hardware_context: None,
            startup: Some(Intel22000FirmwareStartup::new(io)),
            scan_uid: None,
            tx_queue_id: None,
            state: Ax201DriverState::Prepared,
        })
    }

    pub const fn state(&self) -> Ax201DriverState {
        self.state
    }

    pub const fn function(&self) -> crate::wifi_hw::SupportedWifiFunction {
        self.function
    }

    pub const fn tx_queue_id(&self) -> Option<u8> {
        self.tx_queue_id
    }

    pub fn configure_queues(
        &mut self,
        free_rbd: u64,
        used_rbd: u64,
        status: u64,
        command: u64,
        command_count: u8,
    ) -> Result<(), Ax201DriverError> {
        if self.state != Ax201DriverState::Prepared {
            return Err(Ax201DriverError::NotStarted);
        }
        self.context
            .set_rx_queue(free_rbd, used_rbd, status)
            .map_err(Ax201DriverError::Queue)?;
        self.context
            .set_command_queue(command, command_count)
            .map_err(Ax201DriverError::Queue)
    }

    pub fn attach_queues(
        &mut self,
        queues: crate::wifi_hw::Ax201QueueOwner,
    ) -> Result<(), Ax201DriverError> {
        if self.state != Ax201DriverState::Prepared || self.queues.is_some() {
            return Err(Ax201DriverError::NotStarted);
        }
        self.context
            .set_rx_queue(
                queues.free_rbd_address(),
                queues.used_rbd_address(),
                queues.status_address(),
            )
            .map_err(Ax201DriverError::Queue)?;
        self.context
            .set_command_queue(
                queues.command_queue_address(),
                queues.command_queue_context_size(),
            )
            .map_err(Ax201DriverError::Queue)?;
        self.queues = Some(queues);
        Ok(())
    }

    pub fn allocate_and_attach_queues(&mut self) -> Result<(), Ax201DriverError> {
        let queues =
            crate::wifi_hw::Ax201QueueOwner::allocate().map_err(Ax201DriverError::Queues)?;
        self.attach_queues(queues)
    }

    pub fn start(&mut self, mac_id: u16) -> Result<(), Ax201DriverError> {
        if self.state != Ax201DriverState::Prepared {
            return Err(Ax201DriverError::NotStarted);
        }
        if self.queues.is_none() {
            return Err(Ax201DriverError::NotStarted);
        }
        let hardware_context = self
            .context
            .build_hardware_context(mac_id)
            .map_err(Ax201DriverError::Context)?;
        let mut startup = self.startup.take().ok_or(Ax201DriverError::NotStarted)?;
        if let Err(error) = startup.publish_context(&hardware_context) {
            self.state = Ax201DriverState::Failed;
            return Err(Ax201DriverError::Startup(error));
        }
        let queues = self.queues.as_ref().ok_or(Ax201DriverError::NotStarted)?;
        if let Err(error) = startup.program_rx_queue(
            queues.free_rbd_address(),
            queues.used_rbd_address(),
            queues.status_address(),
        ) {
            self.state = Ax201DriverState::Failed;
            return Err(Ax201DriverError::Startup(error));
        }
        if let Err(error) = startup.ring_rx_queue(crate::wifi_hw::AX201_RX_RING_ENTRIES as u16) {
            self.state = Ax201DriverState::Failed;
            return Err(Ax201DriverError::Startup(error));
        }
        if let Err(error) = startup.issue_cpu_init_run() {
            self.state = Ax201DriverState::Failed;
            return Err(Ax201DriverError::Startup(error));
        }
        startup.begin_alive_wait().map_err(|error| {
            self.state = Ax201DriverState::Failed;
            Ax201DriverError::Startup(error)
        })?;
        self.hardware_context = Some(hardware_context);
        self.startup = Some(startup);
        self.state = Ax201DriverState::AwaitingAlive;
        Ok(())
    }

    pub fn handle_alive(
        &mut self,
        command: u8,
        abi: IntelAliveAbiVersion,
        payload: &[u8],
    ) -> Result<IntelVersionedAliveInfo, IntelAliveError> {
        if self.state != Ax201DriverState::AwaitingAlive {
            return Err(IntelAliveError::InvalidState);
        }
        let result = self
            .startup
            .as_mut()
            .ok_or(IntelAliveError::InvalidState)?
            .handle_alive_notification_versioned(command, abi, payload);
        if result.is_ok() {
            self.state = Ax201DriverState::Running;
        } else {
            self.state = Ax201DriverState::Failed;
        }
        result
    }

    pub fn ring_command_queue(
        &mut self,
        queue_id: u8,
        write_pointer: u16,
    ) -> Result<(), Ax201DriverError> {
        if self.state != Ax201DriverState::Running {
            return Err(Ax201DriverError::NotStarted);
        }
        self.startup
            .as_mut()
            .ok_or(Ax201DriverError::NotStarted)?
            .ring_command_queue(queue_id, write_pointer)
            .map_err(Ax201DriverError::Startup)
    }

    /// Copy and submit one host command through the AX201 non-DQA command
    /// queue. The bounded queue owner retains each DMA payload until the caller
    /// confirms a response with the returned sequence number.
    pub fn post_command(
        &mut self,
        opcode: u8,
        group: u8,
        version: Option<u8>,
        payload: &[u8],
    ) -> Result<u16, Ax201DriverError> {
        if self.state != Ax201DriverState::Running {
            return Err(Ax201DriverError::NotStarted);
        }
        let (sequence, write_index) = self
            .queues
            .as_mut()
            .ok_or(Ax201DriverError::QueueDma)?
            .post_command(opcode, group, version, payload)
            .map_err(Ax201DriverError::Command)?;
        if let Err(error) = self
            .startup
            .as_mut()
            .ok_or(Ax201DriverError::NotStarted)?
            .ring_command_queue(crate::wifi_hw::AX201_COMMAND_QUEUE_ID, write_index)
        {
            self.state = Ax201DriverState::Failed;
            return Err(Ax201DriverError::Startup(error));
        }
        Ok(sequence)
    }

    /// Complete the single bounded outstanding command only after its RX
    /// response carries the exact sequence returned by `post_command`.
    pub fn complete_command(&mut self, sequence: u16) -> Result<(), Ax201DriverError> {
        if self.state != Ax201DriverState::Running {
            return Err(Ax201DriverError::NotStarted);
        }
        self.queues
            .as_mut()
            .ok_or(Ax201DriverError::QueueDma)?
            .complete_command(sequence)
            .map_err(Ax201DriverError::Command)
    }

    /// Enable Intel's dynamic queue allocation mode through the normal host
    /// command path. The returned sequence must be acknowledged before a
    /// scheduler queue is allocated.
    pub fn post_dqa_enable(&mut self) -> Result<u16, Ax201DriverError> {
        let mut payload = [0u8; AX201_DQA_ENABLE_PAYLOAD_BYTES];
        let length = encode_ax201_dqa_enable(&mut payload).map_err(Ax201DriverError::Tx)?;
        self.post_command(
            INTEL_DQA_ENABLE_CMD,
            INTEL_DATA_PATH_GROUP,
            Some(1),
            &payload[..length],
        )
    }

    /// Allocate one firmware scheduler queue for a station/TID pair using the
    /// queue owner's real byte-count table and TFD ring addresses.
    pub fn post_tx_queue_config(&mut self, sta_id: u8, tid: u8) -> Result<u16, Ax201DriverError> {
        let Some(sta_mask) = 1u32.checked_shl(u32::from(sta_id)) else {
            return Err(Ax201DriverError::Tx(Ax201TxError::InvalidQueue));
        };
        let queues = self.queues.as_ref().ok_or(Ax201DriverError::QueueDma)?;
        let mut payload = [0u8; AX201_SCD_QUEUE_CONFIG_V3_BYTES];
        let length = encode_ax201_scd_queue_config_v3(
            sta_mask,
            tid,
            0,
            2,
            queues.tx_byte_count_address(),
            queues.tx_queue_address(),
            &mut payload,
        )
        .map_err(Ax201DriverError::Tx)?;
        self.post_command(
            INTEL_SCD_QUEUE_CONFIG_CMD,
            INTEL_DATA_PATH_GROUP,
            Some(INTEL_SCD_QUEUE_CONFIG_VERSION_V3),
            &payload[..length],
        )
    }

    /// Submit an encoded Intel TX command through the dedicated AX201 data
    /// queue. The caller must have selected a queue from the firmware's DQA
    /// configuration; this method never routes frames through queue 9.
    pub fn post_tx_payload(
        &mut self,
        queue_id: u8,
        payload: &[u8],
    ) -> Result<u16, Ax201DriverError> {
        if self.state != Ax201DriverState::Running {
            return Err(Ax201DriverError::NotStarted);
        }
        let (sequence, write_index) = self
            .queues
            .as_mut()
            .ok_or(Ax201DriverError::QueueDma)?
            .post_tx_payload(queue_id, payload)
            .map_err(Ax201DriverError::Queues)?;
        if let Err(error) = self
            .startup
            .as_mut()
            .ok_or(Ax201DriverError::NotStarted)?
            .ring_command_queue(queue_id, write_index)
        {
            self.state = Ax201DriverState::Failed;
            return Err(Ax201DriverError::Startup(error));
        }
        Ok(sequence)
    }

    /// Reclaim a data-TX payload only after the matching TX completion has
    /// been decoded by the notification path.
    pub fn complete_tx(&mut self, sequence: u16) -> Result<(), Ax201DriverError> {
        if self.state != Ax201DriverState::Running {
            return Err(Ax201DriverError::NotStarted);
        }
        self.queues
            .as_mut()
            .ok_or(Ax201DriverError::QueueDma)?
            .complete_tx(sequence)
            .map_err(Ax201DriverError::Queues)
    }

    pub fn accept_tx_queue_config_response(
        &mut self,
        bytes: &[u8],
    ) -> Result<Ax201TxQueueConfigResponse, Ax201DriverError> {
        let notification =
            parse_intel_rx_notification(bytes).map_err(Ax201DriverError::CommandResponse)?;
        if notification.group_id != INTEL_DATA_PATH_GROUP
            || notification.command != INTEL_SCD_QUEUE_CONFIG_CMD
        {
            return Err(Ax201DriverError::Tx(Ax201TxError::WrongCommand));
        }
        let response = parse_ax201_tx_queue_config_response(notification.payload)
            .map_err(Ax201DriverError::Tx)?;
        self.queues
            .as_mut()
            .ok_or(Ax201DriverError::QueueDma)?
            .set_tx_write_index(response.write_pointer)
            .map_err(Ax201DriverError::Queues)?;
        self.tx_queue_id = Some(response.queue_number as u8);
        Ok(response)
    }

    pub fn post_tx_management_payload(&mut self, payload: &[u8]) -> Result<u16, Ax201DriverError> {
        let queue_id = self
            .tx_queue_id
            .ok_or(Ax201DriverError::Tx(Ax201TxError::InvalidQueue))?;
        self.post_tx_payload(queue_id, payload)
    }

    /// Decode a direct `REPLY_TX` notification and reclaim the exact TFD
    /// slot carried in its RX command-header sequence.
    pub fn complete_tx_response(
        &mut self,
        bytes: &[u8],
    ) -> Result<Ax201TxResponse, Ax201DriverError> {
        let notification =
            parse_intel_rx_notification(bytes).map_err(Ax201DriverError::CommandResponse)?;
        let response = parse_ax201_tx_response(notification).map_err(Ax201DriverError::Tx)?;
        self.complete_tx(response.sequence)?;
        Ok(response)
    }

    /// Parse a device response and retire the retained command DMA only when
    /// its sequence matches the outstanding host command. Unsolicited RX
    /// notifications (sequence bit 15) therefore cannot reclaim command
    /// storage accidentally.
    pub fn complete_command_response<'a>(
        &mut self,
        bytes: &'a [u8],
    ) -> Result<IntelRxNotification<'a>, Ax201DriverError> {
        let notification =
            parse_intel_rx_notification(bytes).map_err(Ax201DriverError::CommandResponse)?;
        if notification.sequence & 0x8000 != 0 {
            return Err(Ax201DriverError::Command(
                crate::wifi_hw::Ax201QueueError::CommandSequence,
            ));
        }
        self.complete_command(notification.sequence)?;
        Ok(notification)
    }

    /// Parse and retire a direct command response only when its generic
    /// 32-bit firmware status is present and successful. Unsolicited
    /// notifications remain on the dedicated RX-notification paths.
    pub fn complete_successful_command_response<'a>(
        &mut self,
        bytes: &'a [u8],
    ) -> Result<IntelRxNotification<'a>, Ax201DriverError> {
        let notification = parse_intel_rx_notification(bytes)
            .map_err(Ax201CommandStatusError::Notification)
            .map_err(Ax201DriverError::CommandStatus)?;
        if notification.sequence & 0x8000 != 0 {
            return Err(Ax201DriverError::CommandStatus(
                Ax201CommandStatusError::Unsolicited,
            ));
        }
        let status = notification
            .payload
            .get(0..4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes);
        self.complete_command(notification.sequence)?;
        let Some(status) = status else {
            return Err(Ax201DriverError::CommandStatus(
                Ax201CommandStatusError::MissingStatus,
            ));
        };
        if status != 0 {
            return Err(Ax201DriverError::CommandStatus(
                Ax201CommandStatusError::Firmware(status),
            ));
        }
        Ok(notification)
    }

    /// Submit the bounded scan configuration command. The scan request itself
    /// is intentionally a separate, firmware-versioned operation; callers
    /// must negotiate that command version before issuing it.
    pub fn post_scan_config(&mut self, config: Ax201ScanConfig) -> Result<u16, Ax201DriverError> {
        let mut payload = [0u8; Ax201ScanConfig::PAYLOAD_BYTES];
        config
            .encode(&mut payload)
            .map_err(Ax201DriverError::Scan)?;
        self.post_command(
            INTEL_SCAN_CONFIG_CMD,
            INTEL_SCAN_GROUP,
            Some(INTEL_SCAN_CONFIG_VERSION),
            &payload,
        )
    }

    /// Submit the API-1 legacy PHY context command. This is kept separate
    /// from scan submission so a caller must explicitly establish and
    /// complete the PHY context before enabling radio operations.
    pub fn post_phy_context_v1(
        &mut self,
        context: Ax201PhyContextV1,
    ) -> Result<u16, Ax201DriverError> {
        let mut payload = [0u8; Ax201PhyContextV1::PAYLOAD_BYTES];
        context
            .encode(&mut payload)
            .map_err(Ax201DriverError::PhyContext)?;
        self.post_command(INTEL_PHY_CONTEXT_CMD, INTEL_LEGACY_GROUP, Some(1), &payload)
    }

    pub fn post_mac_context_sta_v1(
        &mut self,
        context: Ax201MacContextStaV1,
    ) -> Result<u16, Ax201DriverError> {
        let mut payload = [0u8; Ax201MacContextStaV1::PAYLOAD_BYTES];
        context
            .encode(&mut payload)
            .map_err(Ax201DriverError::MacContext)?;
        self.post_command(INTEL_MAC_CONTEXT_CMD, INTEL_LEGACY_GROUP, Some(1), &payload)
    }

    pub fn post_binding_context_v2(
        &mut self,
        context: Ax201BindingContextV2,
    ) -> Result<u16, Ax201DriverError> {
        let mut payload = [0u8; Ax201BindingContextV2::PAYLOAD_BYTES];
        context
            .encode(&mut payload)
            .map_err(Ax201DriverError::BindingContext)?;
        self.post_command(
            INTEL_BINDING_CONTEXT_CMD,
            INTEL_LEGACY_GROUP,
            Some(2),
            &payload,
        )
    }

    /// Submit the explicit API-17 passive UMAC scan request. The caller owns
    /// the channel policy; this method only emits the bounded checked wire
    /// payload and retains it through the normal command completion path.
    pub fn post_scan_request_v17(
        &mut self,
        uid: u32,
        channels: &[Ax201ScanChannel],
        passive_dwell: u8,
    ) -> Result<u16, Ax201DriverError> {
        if self.scan_uid.is_some() {
            return Err(Ax201DriverError::Scan(Ax201ScanError::ScanAlreadyActive));
        }
        let mut payload = [0u8; AX201_SCAN_REQUEST_V17_BYTES];
        encode_ax201_scan_request_v17(uid, channels, passive_dwell, &mut payload)
            .map_err(Ax201DriverError::Scan)?;
        let sequence = self.post_command(
            INTEL_SCAN_REQUEST_UMAC_CMD,
            INTEL_SCAN_GROUP,
            Some(INTEL_SCAN_REQUEST_UMAC_VERSION_V17),
            &payload,
        )?;
        self.scan_uid = Some(uid);
        Ok(sequence)
    }

    /// Submit an API-17 active scan. The probe template and directed SSID
    /// list are copied into the owned command payload before queueing.
    pub fn post_active_scan_request_v17(
        &mut self,
        uid: u32,
        channels: &[Ax201ScanChannel],
        active_dwell: u8,
        transmitter: [u8; 6],
        ssids: &[&[u8]],
    ) -> Result<u16, Ax201DriverError> {
        if self.scan_uid.is_some() {
            return Err(Ax201DriverError::Scan(Ax201ScanError::ScanAlreadyActive));
        }
        let mut payload = [0u8; AX201_SCAN_REQUEST_V17_BYTES];
        encode_ax201_active_scan_request_v17(
            uid,
            channels,
            active_dwell,
            transmitter,
            ssids,
            &mut payload,
        )
        .map_err(Ax201DriverError::Scan)?;
        let sequence = self.post_command(
            INTEL_SCAN_REQUEST_UMAC_CMD,
            INTEL_SCAN_GROUP,
            Some(INTEL_SCAN_REQUEST_UMAC_VERSION_V17),
            &payload,
        )?;
        self.scan_uid = Some(uid);
        Ok(sequence)
    }

    pub fn post_scan_abort(&mut self, uid: u32) -> Result<u16, Ax201DriverError> {
        match self.scan_uid {
            None => return Err(Ax201DriverError::Scan(Ax201ScanError::ScanNotActive)),
            Some(active_uid) if active_uid != uid => {
                return Err(Ax201DriverError::Scan(Ax201ScanError::ScanUidMismatch));
            }
            Some(_) => {}
        }
        let mut payload = [0u8; 8];
        encode_ax201_scan_abort(uid, &mut payload).map_err(Ax201DriverError::Scan)?;
        self.post_command(
            INTEL_SCAN_ABORT_UMAC_CMD,
            INTEL_SCAN_GROUP,
            Some(INTEL_SCAN_ABORT_VERSION),
            &payload,
        )
    }

    /// Clear a scan session only after the caller has received a successful
    /// response to the matching abort command. Keeping this acknowledgement
    /// separate preserves retryability when abort submission or completion
    /// fails.
    pub fn finish_scan_abort(&mut self, uid: u32) -> Result<(), Ax201DriverError> {
        if self.scan_uid != Some(uid) {
            return Err(Ax201DriverError::Scan(Ax201ScanError::ScanUidMismatch));
        }
        self.scan_uid = None;
        Ok(())
    }

    /// Route an unsolicited scan notification only to the currently owned
    /// UID. A completion retires the session; an iteration leaves it active.
    pub fn handle_scan_notification(
        &mut self,
        bytes: &[u8],
    ) -> Result<Ax201ScanNotification, Ax201DriverError> {
        let notification =
            parse_intel_rx_notification(bytes).map_err(Ax201DriverError::CommandResponse)?;
        if notification.sequence & 0x8000 == 0 {
            return Err(Ax201DriverError::Scan(Ax201ScanError::WrongCommand));
        }
        let active_uid = self
            .scan_uid
            .ok_or(Ax201DriverError::Scan(Ax201ScanError::ScanNotActive))?;
        if notification.command == INTEL_SCAN_COMPLETE_UMAC_CMD {
            let complete =
                parse_ax201_scan_complete(notification).map_err(Ax201DriverError::Scan)?;
            if complete.uid != active_uid {
                return Err(Ax201DriverError::Scan(Ax201ScanError::ScanUidMismatch));
            }
            self.scan_uid = None;
            Ok(Ax201ScanNotification::Complete(complete))
        } else if notification.command == INTEL_SCAN_ITERATION_COMPLETE_UMAC_CMD {
            let iteration = parse_ax201_scan_iteration_complete(notification)
                .map_err(Ax201DriverError::Scan)?;
            if iteration.uid != active_uid {
                return Err(Ax201DriverError::Scan(Ax201ScanError::ScanUidMismatch));
            }
            Ok(Ax201ScanNotification::Iteration(iteration))
        } else {
            Err(Ax201DriverError::Scan(Ax201ScanError::WrongCommand))
        }
    }

    pub fn read_rx_notification(
        &self,
        ring_index: usize,
        out: &mut [u8],
    ) -> Result<Option<usize>, Ax201DriverError> {
        if out.len() < crate::wifi_hw::AX201_RX_BUFFER_BYTES as usize {
            return Err(Ax201DriverError::QueueDma);
        }
        if self.state != Ax201DriverState::AwaitingAlive && self.state != Ax201DriverState::Running
        {
            return Err(Ax201DriverError::NotStarted);
        }
        let queues = self.queues.as_ref().ok_or(Ax201DriverError::QueueDma)?;
        let closed = usize::from(
            queues
                .closed_rbd_index()
                .map_err(|_| Ax201DriverError::QueueDma)?,
        );
        if ring_index >= closed || ring_index >= crate::wifi_hw::AX201_RX_RING_ENTRIES {
            return Ok(None);
        }
        let rbid = queues
            .validated_used_rbid(ring_index)
            .map_err(|_| Ax201DriverError::QueueDma)?;
        queues
            .read_rx_buffer(
                rbid,
                &mut out[..crate::wifi_hw::AX201_RX_BUFFER_BYTES as usize],
            )
            .map_err(|_| Ax201DriverError::QueueDma)?;
        let packet_len =
            usize::try_from(alive_u32(&out[..4], 0).unwrap_or(0) & INTEL_RX_FRAME_SIZE_MASK)
                .map_err(|_| Ax201DriverError::QueueDma)?
                .checked_add(INTEL_RX_PACKET_PREFIX_SIZE)
                .ok_or(Ax201DriverError::QueueDma)?;
        if packet_len > out.len() {
            return Err(Ax201DriverError::QueueDma);
        }
        Ok(Some(packet_len))
    }

    /// Consume one device-produced RX completion and return its buffer to the
    /// free ring. The RFH write pointer is advanced only on the hardware's
    /// eight-descriptor cadence; descriptors between doorbells are already
    /// published in DMA memory and are included by the next doorbell.
    pub fn poll_rx_notification(
        &mut self,
        out: &mut [u8],
    ) -> Result<Option<usize>, Ax201DriverError> {
        if out.len() < crate::wifi_hw::AX201_RX_BUFFER_BYTES as usize {
            return Err(Ax201DriverError::QueueDma);
        }
        if self.state != Ax201DriverState::AwaitingAlive && self.state != Ax201DriverState::Running
        {
            return Err(Ax201DriverError::NotStarted);
        }
        let (packet_len, new_write_index) = {
            let queues = self.queues.as_mut().ok_or(Ax201DriverError::QueueDma)?;
            let Some((_ring_index, rbid)) = queues
                .take_next_completion()
                .map_err(|_| Ax201DriverError::QueueDma)?
            else {
                return Ok(None);
            };
            queues
                .read_rx_buffer(
                    rbid,
                    &mut out[..crate::wifi_hw::AX201_RX_BUFFER_BYTES as usize],
                )
                .map_err(|_| Ax201DriverError::QueueDma)?;
            let packet_len =
                usize::try_from(alive_u32(&out[..4], 0).unwrap_or(0) & INTEL_RX_FRAME_SIZE_MASK)
                    .map_err(|_| Ax201DriverError::QueueDma)?
                    .checked_add(INTEL_RX_PACKET_PREFIX_SIZE)
                    .ok_or(Ax201DriverError::QueueDma)?;
            let new_write_index = queues
                .recycle_rx_buffer(rbid)
                .map_err(|_| Ax201DriverError::QueueDma)?;
            (packet_len, new_write_index)
        };
        if packet_len > out.len() {
            return Err(Ax201DriverError::QueueDma);
        }
        if new_write_index & 7 == 0 {
            self.startup
                .as_mut()
                .ok_or(Ax201DriverError::NotStarted)?
                .ring_rx_queue(new_write_index)
                .map_err(Ax201DriverError::Startup)?;
        }
        Ok(Some(packet_len))
    }

    pub fn dispatch_rx_notification(
        &mut self,
        abi: IntelAliveAbiVersion,
        bytes: &[u8],
    ) -> Result<IntelRxDispatchResult, IntelRxNotificationError> {
        if self.state != Ax201DriverState::AwaitingAlive {
            return Err(IntelRxNotificationError::Alive(
                IntelAliveError::InvalidState,
            ));
        }
        let result = self
            .startup
            .as_mut()
            .ok_or(IntelRxNotificationError::Alive(
                IntelAliveError::InvalidState,
            ))?
            .dispatch_rx_notification(abi, bytes);
        if matches!(result, Ok(IntelRxDispatchResult::Alive(_))) {
            self.state = Ax201DriverState::Running;
        }
        result
    }

    /// Drop hardware-visible context only after the caller has disabled MSI
    /// and stopped the device. Queue storage is released only after the queue
    /// owner has been marked quiesced; otherwise it is retained deliberately.
    pub fn stop(&mut self) {
        self.startup = None;
        self.hardware_context = None;
        self.tx_queue_id = None;
        if self
            .queues
            .as_ref()
            .is_some_and(crate::wifi_hw::Ax201QueueOwner::hardware_quiesced)
        {
            self.queues = None;
        }
        self.state = Ax201DriverState::Stopped;
    }

    pub fn mark_hardware_quiesced(&mut self) {
        if let Some(queues) = self.queues.as_mut() {
            queues.mark_hardware_quiesced();
        }
    }
}

struct Stage13_10sMockIo {
    context_offset: u32,
    context_address: u64,
    context_writes: usize,
    prph_register: u32,
    prph_value: u32,
    prph_writes: usize,
    fail_prph: bool,
}
impl IntelContextInfoPublicationIo for Stage13_10sMockIo {
    fn write_context_info_base(&mut self, o: u32, a: u64) -> Result<(), IntelContextInfoAbiError> {
        self.context_offset = o;
        self.context_address = a;
        self.context_writes += 1;
        Ok(())
    }
}
impl IntelFirmwareStartupIo for Stage13_10sMockIo {
    fn write_prph(&mut self, r: u32, v: u32) -> Result<(), IntelFirmwareStartError> {
        if self.fail_prph {
            return Err(IntelFirmwareStartError::Prph);
        }
        self.prph_register = r;
        self.prph_value = v;
        self.prph_writes += 1;
        Ok(())
    }
}
// === Stage 13.10T: Intel 22000/AX200 ALIVE notification validation ===
pub const INTEL_UCODE_ALIVE_NTFY: u8 = 0x01;
/// Read/configure operational statistics after runtime ALIVE. The command is
/// intentionally used only as a post-ALIVE transport probe by the physical
/// image; it is not sent while the boot image is still in initialization.
pub const INTEL_STATISTICS_CMD: u8 = 0x9c;
pub const INTEL_STATISTICS_FLG_DISABLE_NOTIF: u32 = 0x2;
pub const INTEL_LEGACY_GROUP: u8 = 0x00;
pub const INTEL_PHY_CONTEXT_CMD: u8 = 0x08;
pub const INTEL_MAC_CONTEXT_CMD: u8 = 0x28;
pub const INTEL_BINDING_CONTEXT_CMD: u8 = 0x2b;
/// Intel 22000 scan-group command IDs. The command version is part of the
/// firmware ABI and is supplied explicitly by the caller; API 77 alone does
/// not imply one scan-command version.
pub const INTEL_SCAN_GROUP: u8 = 0x06;
pub const INTEL_SCAN_CONFIG_CMD: u8 = 0x0c;
pub const INTEL_SCAN_REQUEST_UMAC_CMD: u8 = 0x0d;
/// The QuZ UMAC request layout is selected by command-version negotiation;
/// this is the explicit v17 encoder, not an inference from the firmware API
/// number.
pub const INTEL_SCAN_REQUEST_UMAC_VERSION_V17: u8 = 17;
pub const INTEL_SCAN_ABORT_UMAC_CMD: u8 = 0x0e;
pub const INTEL_SCAN_COMPLETE_UMAC_CMD: u8 = 0x0f;
pub const INTEL_SCAN_ITERATION_COMPLETE_UMAC_CMD: u8 = 0xb5;
pub const INTEL_SCAN_CONFIG_VERSION: u8 = 5;
pub const INTEL_SCAN_ABORT_VERSION: u8 = 1;
pub const INTEL_TX_CMD: u8 = 0x1c;
pub const INTEL_TX_CMD_VERSION_V9: u8 = 9;
pub const INTEL_DATA_PATH_GROUP: u8 = 0x05;
pub const INTEL_DQA_ENABLE_CMD: u8 = 0x00;
pub const INTEL_SCD_QUEUE_CONFIG_CMD: u8 = 0x17;
pub const INTEL_SCD_QUEUE_CONFIG_VERSION_V3: u8 = 3;
pub const AX201_TX_CMD_V9_HEADER_BYTES: usize = 20;
pub const AX201_TX_MAX_FRAME_BYTES: usize = 2342;
pub const AX201_DQA_ENABLE_PAYLOAD_BYTES: usize = 4;
pub const AX201_SCD_QUEUE_CONFIG_V3_BYTES: usize = 36;
pub const AX201_TX_QUEUE_RESPONSE_BYTES: usize = 8;
pub const AX201_TX_RESPONSE_BYTES: usize = 41;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201ScanError {
    InvalidChains,
    InvalidUid,
    InvalidChannels,
    UnsupportedSsid,
    InvalidProbeAddress,
    ScanAlreadyActive,
    ScanNotActive,
    ScanUidMismatch,
    WrongCommand,
    InvalidPayload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201TxError {
    EmptyFrame,
    FrameTooLarge,
    InvalidManagementFrame,
    BufferTooSmall,
    InvalidQueue,
    InvalidDmaAddress,
    InvalidQueueSize,
    InvalidResponse,
    WrongCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201TxManagementOptions {
    pub rate_n_flags: u32,
    pub high_priority: bool,
}

/// Encode the 22000-family TX_CMD API v9 header followed by one complete
/// management MPDU. This is deliberately an encoder boundary only: TX_CMD
/// must be published through a dedicated data-TX queue, not the host-command
/// queue used by `post_command`.
pub fn encode_ax201_tx_management_v9(
    frame: &[u8],
    options: Ax201TxManagementOptions,
    output: &mut [u8],
) -> Result<usize, Ax201TxError> {
    if frame.is_empty() {
        return Err(Ax201TxError::EmptyFrame);
    }
    if frame.len() > AX201_TX_MAX_FRAME_BYTES {
        return Err(Ax201TxError::FrameTooLarge);
    }
    let Ok(header) = crate::wifi80211::parse_management_header(frame) else {
        return Err(Ax201TxError::InvalidManagementFrame);
    };
    if header.control.management_subtype().is_none() {
        return Err(Ax201TxError::InvalidManagementFrame);
    }
    let total = AX201_TX_CMD_V9_HEADER_BYTES
        .checked_add(frame.len())
        .ok_or(Ax201TxError::FrameTooLarge)?;
    if output.len() < total {
        return Err(Ax201TxError::BufferTooSmall);
    }
    output[..total].fill(0);
    output[0..2].copy_from_slice(&(frame.len() as u16).to_le_bytes());
    let mut flags = 1u32; // IWL_TX_FLAGS_CMD_RATE
    if options.high_priority {
        flags |= 1 << 2; // IWL_TX_FLAGS_HIGH_PRI
    }
    // offload_assist and dram_info remain zero in this management-frame path.
    output[4..8].copy_from_slice(&flags.to_le_bytes());
    output[16..20].copy_from_slice(&options.rate_n_flags.to_le_bytes());
    output[AX201_TX_CMD_V9_HEADER_BYTES..total].copy_from_slice(frame);
    Ok(total)
}

/// Encode the API-1 DQA enable command. Intel's 22000 firmware uses the
/// command queue number as a little-endian u32; it is sent through the wide
/// DATA_PATH group, not as a TX frame.
pub fn encode_ax201_dqa_enable(output: &mut [u8]) -> Result<usize, Ax201TxError> {
    if output.len() < AX201_DQA_ENABLE_PAYLOAD_BYTES {
        return Err(Ax201TxError::BufferTooSmall);
    }
    output[..AX201_DQA_ENABLE_PAYLOAD_BYTES]
        .copy_from_slice(&(u32::from(crate::wifi_hw::AX201_COMMAND_QUEUE_ID)).to_le_bytes());
    Ok(AX201_DQA_ENABLE_PAYLOAD_BYTES)
}

/// Encode the 22000 DATA_PATH `SCD_QUEUE_CONFIG_CMD` add form. The queue is
/// allocated by firmware for the station/TID pair; the returned queue number
/// must be used for subsequent TFD doorbells.
pub fn encode_ax201_scd_queue_config_v3(
    sta_mask: u32,
    tid: u8,
    flags: u32,
    cb_size: u32,
    byte_count_address: u64,
    tfd_queue_address: u64,
    output: &mut [u8],
) -> Result<usize, Ax201TxError> {
    if sta_mask == 0 || tid > 15 {
        return Err(Ax201TxError::InvalidQueue);
    }
    if cb_size > 5 || cb_size == 0 {
        return Err(Ax201TxError::InvalidQueueSize);
    }
    if byte_count_address >> 36 != 0 || tfd_queue_address >> 36 != 0 {
        return Err(Ax201TxError::InvalidDmaAddress);
    }
    if output.len() < AX201_SCD_QUEUE_CONFIG_V3_BYTES {
        return Err(Ax201TxError::BufferTooSmall);
    }
    output[..AX201_SCD_QUEUE_CONFIG_V3_BYTES].fill(0);
    output[0..4].copy_from_slice(&0u32.to_le_bytes()); // IWL_SCD_QUEUE_ADD
    output[4..8].copy_from_slice(&sta_mask.to_le_bytes());
    output[8] = tid;
    output[12..16].copy_from_slice(&flags.to_le_bytes());
    output[16..20].copy_from_slice(&cb_size.to_le_bytes());
    output[20..28].copy_from_slice(&byte_count_address.to_le_bytes());
    output[28..36].copy_from_slice(&tfd_queue_address.to_le_bytes());
    Ok(AX201_SCD_QUEUE_CONFIG_V3_BYTES)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201TxQueueConfigResponse {
    pub queue_number: u16,
    pub write_pointer: u16,
}

pub fn parse_ax201_tx_queue_config_response(
    payload: &[u8],
) -> Result<Ax201TxQueueConfigResponse, Ax201TxError> {
    if payload.len() != AX201_TX_QUEUE_RESPONSE_BYTES {
        return Err(Ax201TxError::InvalidResponse);
    }
    let queue_number = u16::from_le_bytes([payload[0], payload[1]]);
    let flags = u16::from_le_bytes([payload[2], payload[3]]);
    let write_pointer = u16::from_le_bytes([payload[4], payload[5]]);
    let reserved = u16::from_le_bytes([payload[6], payload[7]]);
    let valid_queue = (crate::wifi_hw::AX201_MANAGEMENT_TX_QUEUE_MIN
        ..=crate::wifi_hw::AX201_MANAGEMENT_TX_QUEUE_MAX)
        .contains(&(queue_number as u8))
        || (crate::wifi_hw::AX201_DATA_TX_QUEUE_MIN..=crate::wifi_hw::AX201_DATA_TX_QUEUE_MAX)
            .contains(&(queue_number as u8));
    if queue_number > u16::from(u8::MAX)
        || !valid_queue
        || flags != 0
        || write_pointer >= u16::from(crate::wifi_hw::AX201_DATA_TX_QUEUE_ENTRIES)
        || reserved != 0
    {
        return Err(Ax201TxError::InvalidResponse);
    }
    Ok(Ax201TxQueueConfigResponse {
        queue_number,
        write_pointer,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201TxResponse {
    pub frame_count: u8,
    pub tfd_info: u32,
    pub byte_count: u16,
    pub frame_control: u16,
    pub queue_number: u16,
    pub status: u16,
    pub sequence: u16,
}

/// Decode the API 9 `REPLY_TX` payload. The current owner submits one frame
/// at a time, so aggregated responses are rejected until their per-frame
/// reclamation policy is implemented.
pub fn parse_ax201_tx_response(
    notification: IntelRxNotification<'_>,
) -> Result<Ax201TxResponse, Ax201TxError> {
    if notification.group_id != INTEL_LEGACY_GROUP || notification.command != INTEL_TX_CMD {
        return Err(Ax201TxError::WrongCommand);
    }
    let payload = notification.payload;
    if payload.len() != AX201_TX_RESPONSE_BYTES || payload[0] != 1 {
        return Err(Ax201TxError::InvalidResponse);
    }
    let queue_number = u16::from_le_bytes([payload[33], payload[34]]);
    let valid_queue = (crate::wifi_hw::AX201_MANAGEMENT_TX_QUEUE_MIN
        ..=crate::wifi_hw::AX201_MANAGEMENT_TX_QUEUE_MAX)
        .contains(&(queue_number as u8))
        || (crate::wifi_hw::AX201_DATA_TX_QUEUE_MIN..=crate::wifi_hw::AX201_DATA_TX_QUEUE_MAX)
            .contains(&(queue_number as u8));
    let sequence_queue = (notification.sequence >> 8) & 0x1f;
    let sequence_index = notification.sequence & 0xff;
    if queue_number > u16::from(u8::MAX)
        || !valid_queue
        || notification.sequence & 0x8000 != 0
        || sequence_queue != queue_number
        || sequence_index >= u16::from(crate::wifi_hw::AX201_DATA_TX_QUEUE_ENTRIES)
    {
        return Err(Ax201TxError::InvalidResponse);
    }
    Ok(Ax201TxResponse {
        frame_count: payload[0],
        tfd_info: u32::from_le_bytes(payload[21..25].try_into().unwrap()),
        byte_count: u16::from_le_bytes([payload[27], payload[28]]),
        frame_control: u16::from_le_bytes([payload[31], payload[32]]),
        queue_number,
        status: u16::from_le_bytes([payload[37], payload[38]]),
        sequence: notification.sequence,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201PhyContextError {
    Channel,
    Band,
    Width,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201MacContextError {
    InvalidNodeAddress,
    InvalidTsf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201BindingContextError {
    InvalidMacCount,
}

/// The API-1 legacy PHY_CONTEXT_CMD payload. The values are deliberately
/// represented as the firmware's packed integers; policy (add/modify/remove,
/// chain selection, and timing) belongs to the caller above this wire layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201PhyContextV1 {
    pub id_and_color: u32,
    pub action: u32,
    pub apply_time: u32,
    pub tx_param_color: u32,
    pub channel: u32,
    pub band: u8,
    pub width: u8,
    pub ctrl_pos: u8,
    pub tx_chain_info: u32,
    pub rx_chain_info: u32,
    pub acquisition_data: u32,
    pub dsp_cfg_flags: u32,
}

impl Ax201PhyContextV1 {
    pub const PAYLOAD_BYTES: usize = 40;

    pub fn encode(
        self,
        output: &mut [u8; Self::PAYLOAD_BYTES],
    ) -> Result<(), Ax201PhyContextError> {
        if self.channel == 0 {
            return Err(Ax201PhyContextError::Channel);
        }
        if self.band > 2 {
            return Err(Ax201PhyContextError::Band);
        }
        if self.width > 4 {
            return Err(Ax201PhyContextError::Width);
        }
        output.fill(0);
        output[0..4].copy_from_slice(&self.id_and_color.to_le_bytes());
        output[4..8].copy_from_slice(&self.action.to_le_bytes());
        output[8..12].copy_from_slice(&self.apply_time.to_le_bytes());
        output[12..16].copy_from_slice(&self.tx_param_color.to_le_bytes());
        output[16..20].copy_from_slice(&self.channel.to_le_bytes());
        output[20] = self.band;
        output[21] = self.width;
        output[22] = self.ctrl_pos;
        output[24..28].copy_from_slice(&self.tx_chain_info.to_le_bytes());
        output[28..32].copy_from_slice(&self.rx_chain_info.to_le_bytes());
        output[32..36].copy_from_slice(&self.acquisition_data.to_le_bytes());
        output[36..40].copy_from_slice(&self.dsp_cfg_flags.to_le_bytes());
        Ok(())
    }
}

/// API-2 BINDING_CONTEXT_CMD. Three MAC slots are part of the fixed firmware
/// record; unused slots must remain zero rather than being omitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201BindingContextV2 {
    pub id_and_color: u32,
    pub action: u32,
    pub macs: [u32; 3],
    pub phy: u32,
    pub lmac_id: u32,
}

impl Ax201BindingContextV2 {
    pub const PAYLOAD_BYTES: usize = 28;

    pub fn encode(
        self,
        output: &mut [u8; Self::PAYLOAD_BYTES],
    ) -> Result<(), Ax201BindingContextError> {
        output.fill(0);
        output[0..4].copy_from_slice(&self.id_and_color.to_le_bytes());
        output[4..8].copy_from_slice(&self.action.to_le_bytes());
        for (index, mac) in self.macs.iter().enumerate() {
            let offset = 8 + index * 4;
            output[offset..offset + 4].copy_from_slice(&mac.to_le_bytes());
        }
        output[20..24].copy_from_slice(&self.phy.to_le_bytes());
        output[24..28].copy_from_slice(&self.lmac_id.to_le_bytes());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201MacQos {
    pub cw_min: u16,
    pub cw_max: u16,
    pub aifsn: u8,
    pub fifos_mask: u8,
    pub edca_txop: u16,
}

/// API-1 managed-station MAC_CONTEXT_CMD. The final 48 bytes are the
/// `iwl_mac_data_p2p_sta`-sized station union; for a normal BSS station the
/// fields are encoded explicitly and the P2P extension remains zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201MacContextStaV1 {
    pub id_and_color: u32,
    pub action: u32,
    pub mac_type: u32,
    pub tsf_id: u32,
    pub node_addr: [u8; 6],
    pub bssid_addr: [u8; 6],
    pub cck_rates: u32,
    pub ofdm_rates: u32,
    pub protection_flags: u32,
    pub cck_short_preamble: u32,
    pub short_slot: u32,
    pub filter_flags: u32,
    pub qos_flags: u32,
    pub ac: [Ax201MacQos; 5],
    pub is_assoc: u32,
    pub dtim_time: u32,
    pub dtim_tsf: u64,
    pub beacon_interval: u32,
    pub reserved: u32,
    pub dtim_interval: u32,
    pub data_policy: u32,
    pub listen_interval: u32,
    pub assoc_id: u32,
    pub assoc_beacon_arrive_time: u32,
    pub p2p_ctwin: u32,
}

impl Ax201MacContextStaV1 {
    pub const PAYLOAD_BYTES: usize = 148;
    pub const BSS_STA_MAC_TYPE: u32 = 5;

    pub fn encode(
        self,
        output: &mut [u8; Self::PAYLOAD_BYTES],
    ) -> Result<(), Ax201MacContextError> {
        if self.node_addr == [0; 6] || self.node_addr == [0xff; 6] || self.node_addr[0] & 1 != 0 {
            return Err(Ax201MacContextError::InvalidNodeAddress);
        }
        if self.tsf_id > 3 {
            return Err(Ax201MacContextError::InvalidTsf);
        }
        output.fill(0);
        let put_u32 = |out: &mut [u8], offset: usize, value: u32| {
            out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        };
        let put_u16 = |out: &mut [u8], offset: usize, value: u16| {
            out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        };
        put_u32(output, 0, self.id_and_color);
        put_u32(output, 4, self.action);
        put_u32(output, 8, self.mac_type);
        put_u32(output, 12, self.tsf_id);
        output[16..22].copy_from_slice(&self.node_addr);
        output[24..30].copy_from_slice(&self.bssid_addr);
        put_u32(output, 32, self.cck_rates);
        put_u32(output, 36, self.ofdm_rates);
        put_u32(output, 40, self.protection_flags);
        put_u32(output, 44, self.cck_short_preamble);
        put_u32(output, 48, self.short_slot);
        put_u32(output, 52, self.filter_flags);
        put_u32(output, 56, self.qos_flags);
        for (index, qos) in self.ac.iter().enumerate() {
            let offset = 60 + index * 8;
            put_u16(output, offset, qos.cw_min);
            put_u16(output, offset + 2, qos.cw_max);
            output[offset + 4] = qos.aifsn;
            output[offset + 5] = qos.fifos_mask;
            put_u16(output, offset + 6, qos.edca_txop);
        }
        let station = 100;
        put_u32(output, station, self.is_assoc);
        put_u32(output, station + 4, self.dtim_time);
        output[station + 8..station + 16].copy_from_slice(&self.dtim_tsf.to_le_bytes());
        put_u32(output, station + 16, self.beacon_interval);
        put_u32(output, station + 20, self.reserved);
        put_u32(output, station + 24, self.dtim_interval);
        put_u32(output, station + 28, self.data_policy);
        put_u32(output, station + 32, self.listen_interval);
        put_u32(output, station + 36, self.assoc_id);
        put_u32(output, station + 40, self.assoc_beacon_arrive_time);
        put_u32(output, station + 44, self.p2p_ctwin);
        Ok(())
    }
}

/// `SCAN_CONFIG_CMD` API version 5, kept as a fixed packed payload so the
/// caller cannot hand the firmware an unbounded or borrowed structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201ScanConfig {
    pub enable_cam_mode: u8,
    pub enable_promiscuous_mode: u8,
    pub broadcast_station_id: u8,
    pub tx_chains: u32,
    pub rx_chains: u32,
}

impl Ax201ScanConfig {
    pub const PAYLOAD_BYTES: usize = 12;

    pub fn encode(self, output: &mut [u8; Self::PAYLOAD_BYTES]) -> Result<(), Ax201ScanError> {
        if self.tx_chains == 0 || self.rx_chains == 0 {
            return Err(Ax201ScanError::InvalidChains);
        }
        output.fill(0);
        output[0] = self.enable_cam_mode;
        output[1] = self.enable_promiscuous_mode;
        output[2] = self.broadcast_station_id;
        output[4..8].copy_from_slice(&self.tx_chains.to_le_bytes());
        output[8..12].copy_from_slice(&self.rx_chains.to_le_bytes());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201ScanComplete {
    pub uid: u32,
    pub last_schedule: u8,
    pub last_iteration: u8,
    pub status: u8,
    pub ebs_status: u8,
    pub time_from_last_iteration: u32,
}

/// The UMAC scan abort payload is exactly `{ uid, flags }`; flags are reserved
/// and must remain zero for this driver boundary.
pub fn encode_ax201_scan_abort(uid: u32, output: &mut [u8; 8]) -> Result<(), Ax201ScanError> {
    if uid == 0 {
        return Err(Ax201ScanError::InvalidUid);
    }
    output.fill(0);
    output[..4].copy_from_slice(&uid.to_le_bytes());
    Ok(())
}

pub const AX201_SCAN_MAX_CHANNELS: usize = 67;
pub const AX201_SCAN_REQUEST_V17_BYTES: usize = 1940;
const AX201_SCAN_PROBE_OFFSET: usize = 596;
const AX201_SCAN_PROBE_HEADER_BYTES: usize = 532;
const AX201_SCAN_DIRECT_SSID_OFFSET: usize =
    AX201_SCAN_PROBE_OFFSET + AX201_SCAN_PROBE_HEADER_BYTES + 4;
const AX201_SCAN_DIRECT_SSID_BYTES: usize = 34;
const AX201_SCAN_DIRECT_SSID_MAX: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201ScanChannel {
    pub number: u8,
    pub band: u8,
    pub flags: u32,
    pub iteration_count: u8,
    pub iteration_interval: u8,
}

/// Encode the API-17 UMAC scan request as a bounded passive scan. The probe
/// request block is intentionally empty: active/direct-SSID probing requires
/// a separately constructed 802.11 probe frame and is rejected here rather
/// than sending an incomplete frame to firmware.
pub fn encode_ax201_scan_request_v17(
    uid: u32,
    channels: &[Ax201ScanChannel],
    passive_dwell: u8,
    output: &mut [u8; AX201_SCAN_REQUEST_V17_BYTES],
) -> Result<(), Ax201ScanError> {
    if uid == 0
        || channels.is_empty()
        || channels.len() > AX201_SCAN_MAX_CHANNELS
        || passive_dwell == 0
    {
        return Err(Ax201ScanError::InvalidChannels);
    }
    if channels.iter().any(|channel| {
        channel.number == 0
            || channel.band > 1
            || channel.iteration_count == 0
            || channel.iteration_interval == 0
    }) {
        return Err(Ax201ScanError::InvalidChannels);
    }
    output.fill(0);
    // uid and out-of-channel priority.
    output[0..4].copy_from_slice(&uid.to_le_bytes());
    // General-parameters v11: force passive + notify each iteration.
    output[8..10].copy_from_slice(&(1u16 << 11 | 1u16 << 2).to_le_bytes());
    output[8 + 3] = 0; // scan_start_mac_or_link_id
    output[8 + 4] = passive_dwell;
    output[8 + 5] = passive_dwell;
    output[8 + 32] = passive_dwell; // passive dwell, low-band
    output[8 + 33] = passive_dwell; // passive dwell, high-band

    // Channel-parameters v7: flags, count, two AP overrides, then 67 x 8.
    let channel_base = 44;
    output[channel_base + 1] = u8::try_from(channels.len()).unwrap_or(0);
    for (index, channel) in channels.iter().enumerate() {
        let offset = channel_base + 4 + index * 8;
        let band_flags = channel.flags | (u32::from(channel.band) << 30);
        output[offset..offset + 4].copy_from_slice(&band_flags.to_le_bytes());
        output[offset + 4] = channel.number;
        // API v5 channel union: PSD20, iteration count, interval.
        output[offset + 6] = channel.iteration_count;
        output[offset + 7] = channel.iteration_interval;
    }
    Ok(())
}

/// Encode an API-17 active scan with a real bounded probe-request template.
///
/// The firmware owns the channel dwell and inserts each directed SSID from
/// `ssids`; the template supplies the management header plus the common
/// supported-rates IEs. An empty SSID list is a normal broadcast-probe scan.
pub fn encode_ax201_active_scan_request_v17(
    uid: u32,
    channels: &[Ax201ScanChannel],
    active_dwell: u8,
    transmitter: [u8; 6],
    ssids: &[&[u8]],
    output: &mut [u8; AX201_SCAN_REQUEST_V17_BYTES],
) -> Result<(), Ax201ScanError> {
    if transmitter == [0; 6]
        || transmitter == [0xff; 6]
        || transmitter[0] & 1 != 0
        || ssids.len() > AX201_SCAN_DIRECT_SSID_MAX
        || ssids
            .iter()
            .any(|ssid| ssid.len() > crate::wifi80211::MAX_SSID_LEN)
    {
        return Err(
            if transmitter == [0; 6] || transmitter == [0xff; 6] || transmitter[0] & 1 != 0 {
                Ax201ScanError::InvalidProbeAddress
            } else {
                Ax201ScanError::UnsupportedSsid
            },
        );
    }
    if active_dwell == 0 {
        return Err(Ax201ScanError::InvalidChannels);
    }
    if uid == 0 || channels.is_empty() || channels.len() > AX201_SCAN_MAX_CHANNELS {
        return Err(Ax201ScanError::InvalidChannels);
    }
    if channels.iter().any(|channel| {
        channel.number == 0
            || channel.band > 1
            || channel.iteration_count == 0
            || channel.iteration_interval == 0
    }) {
        return Err(Ax201ScanError::InvalidChannels);
    }
    output.fill(0);
    output[0..4].copy_from_slice(&uid.to_le_bytes());
    // Notify at every iteration; unlike the passive encoder, FORCE_PASSIVE is
    // clear so the firmware emits broadcast/direct probe requests.
    output[8..10].copy_from_slice(&(1u16 << 2).to_le_bytes());
    output[12] = active_dwell;
    output[13] = active_dwell;
    output[40] = active_dwell;
    output[41] = active_dwell;

    let channel_base = 44;
    output[channel_base + 1] = u8::try_from(channels.len()).unwrap_or(0);
    for (index, channel) in channels.iter().enumerate() {
        let offset = channel_base + 4 + index * 8;
        let flags = channel.flags | (u32::from(channel.band) << 30);
        let flags = if ssids.is_empty() {
            flags
        } else {
            flags | 1u32 << 16
        };
        output[offset..offset + 4].copy_from_slice(&flags.to_le_bytes());
        output[offset + 4] = channel.number;
        output[offset + 6] = channel.iteration_count;
        output[offset + 7] = channel.iteration_interval;
    }

    // iwl_scan_probe_req: mac_header, three band segments, common_data, buf.
    // The common block contains supported rates; the firmware appends the
    // selected direct SSID IE(s) from the array below.
    let probe = AX201_SCAN_PROBE_OFFSET;
    output[probe..probe + 2].copy_from_slice(&0u16.to_le_bytes());
    output[probe + 2..probe + 4].copy_from_slice(&24u16.to_le_bytes());
    let common = probe + 4 * 4;
    output[common..common + 2].copy_from_slice(&24u16.to_le_bytes());
    output[common + 2..common + 4].copy_from_slice(&14u16.to_le_bytes());
    let frame = probe + 20;
    crate::wifi80211::build_management_header(
        &mut output[frame..frame + 24],
        4,
        [0xff; 6],
        transmitter,
        [0xff; 6],
        0,
    )
    .map_err(|_| Ax201ScanError::InvalidPayload)?;
    output[frame + 24..frame + 28].copy_from_slice(&[1, 2, 0x82, 0x84]);
    output[frame + 28..frame + 32].copy_from_slice(&[50, 2, 0x8b, 0x96]);
    output[frame + 32..frame + 34].copy_from_slice(&[0, 0]);
    let direct = AX201_SCAN_DIRECT_SSID_OFFSET;
    for (index, ssid) in ssids.iter().enumerate() {
        let offset = direct + index * AX201_SCAN_DIRECT_SSID_BYTES;
        output[offset] = crate::wifi80211::IE_SSID;
        output[offset + 1] = u8::try_from(ssid.len()).unwrap_or(0);
        output[offset + 2..offset + 2 + ssid.len()].copy_from_slice(ssid);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201ScanIterationComplete {
    pub uid: u32,
    pub scanned_channels: u8,
    pub status: u8,
    pub bluetooth_status: u8,
    pub last_channel: u8,
    pub start_tsf: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201ScanNotification {
    Iteration(Ax201ScanIterationComplete),
    Complete(Ax201ScanComplete),
}

pub fn parse_ax201_scan_iteration_complete(
    notification: IntelRxNotification<'_>,
) -> Result<Ax201ScanIterationComplete, Ax201ScanError> {
    if notification.group_id != INTEL_SCAN_GROUP
        || notification.command != INTEL_SCAN_ITERATION_COMPLETE_UMAC_CMD
    {
        return Err(Ax201ScanError::WrongCommand);
    }
    if notification.payload.len() < 16 {
        return Err(Ax201ScanError::InvalidPayload);
    }
    let scanned_channels = notification.payload[4];
    let result_bytes = usize::from(scanned_channels)
        .checked_mul(8)
        .ok_or(Ax201ScanError::InvalidPayload)?;
    if notification.payload.len() != 16 + result_bytes {
        return Err(Ax201ScanError::InvalidPayload);
    }
    Ok(Ax201ScanIterationComplete {
        uid: u32::from_le_bytes(notification.payload[0..4].try_into().unwrap()),
        scanned_channels,
        status: notification.payload[5],
        bluetooth_status: notification.payload[6],
        last_channel: notification.payload[7],
        start_tsf: u64::from_le_bytes(notification.payload[8..16].try_into().unwrap()),
    })
}
pub const INTEL_ALIVE_STATUS_OK: u16 = 0xCAFE;
pub const INTEL_ALIVE_STATUS_ERR: u16 = 0xDEAD;
pub const INTEL_ALIVE_V3_SIZE: usize = 68;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IntelAliveDiagnostics {
    pub lmac_error_event_table: u32,
    pub lmac_log_event_table: u32,
    pub lmac_cpu_register: u32,
    pub lmac_dbgm_config: u32,
    pub lmac_alive_counter: u32,
    pub lmac_scd_base: u32,
    pub lmac_store_forward_address: u32,
    pub lmac_store_forward_size: u32,
    pub umac_error_info: u32,
    pub umac_debug_print_buffer: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntelAliveInfo {
    pub status: u16,
    pub flags: u16,
    pub lmac_ucode_major: u32,
    pub lmac_ucode_minor: u32,
    pub lmac_ver_subtype: u8,
    pub lmac_ver_type: u8,
    pub lmac_mac: u8,
    pub lmac_opt: u8,
    pub lmac_timestamp: u32,
    pub umac_major: u32,
    pub umac_minor: u32,
    pub diagnostics: IntelAliveDiagnostics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelAliveError {
    InvalidState,
    WrongCommand,
    UnsupportedLength,
    FirmwareRejected(u16),
}

fn alive_u16(payload: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([
        *payload.get(offset)?,
        *payload.get(offset + 1)?,
    ]))
}
fn alive_u32(payload: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *payload.get(offset)?,
        *payload.get(offset + 1)?,
        *payload.get(offset + 2)?,
        *payload.get(offset + 3)?,
    ]))
}

pub fn parse_intel_alive_v3(payload: &[u8]) -> Result<IntelAliveInfo, IntelAliveError> {
    if payload.len() != INTEL_ALIVE_V3_SIZE {
        return Err(IntelAliveError::UnsupportedLength);
    }
    let status = alive_u16(payload, 0).ok_or(IntelAliveError::UnsupportedLength)?;
    let flags = alive_u16(payload, 2).ok_or(IntelAliveError::UnsupportedLength)?;
    let diagnostics = IntelAliveDiagnostics {
        lmac_error_event_table: alive_u32(payload, 20).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_log_event_table: alive_u32(payload, 24).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_cpu_register: alive_u32(payload, 28).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_dbgm_config: alive_u32(payload, 32).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_alive_counter: alive_u32(payload, 36).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_scd_base: alive_u32(payload, 40).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_store_forward_address: alive_u32(payload, 44)
            .ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_store_forward_size: alive_u32(payload, 48)
            .ok_or(IntelAliveError::UnsupportedLength)?,
        umac_error_info: alive_u32(payload, 60).ok_or(IntelAliveError::UnsupportedLength)?,
        umac_debug_print_buffer: alive_u32(payload, 64)
            .ok_or(IntelAliveError::UnsupportedLength)?,
    };
    Ok(IntelAliveInfo {
        status,
        flags,
        lmac_ucode_major: alive_u32(payload, 4).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_ucode_minor: alive_u32(payload, 8).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_ver_subtype: payload[12],
        lmac_ver_type: payload[13],
        lmac_mac: payload[14],
        lmac_opt: payload[15],
        lmac_timestamp: alive_u32(payload, 16).ok_or(IntelAliveError::UnsupportedLength)?,
        umac_major: alive_u32(payload, 52).ok_or(IntelAliveError::UnsupportedLength)?,
        umac_minor: alive_u32(payload, 56).ok_or(IntelAliveError::UnsupportedLength)?,
        diagnostics,
    })
}

impl<I: IntelFirmwareStartupIo> Intel22000FirmwareStartup<I> {
    pub fn handle_alive_notification(
        &mut self,
        command: u8,
        payload: &[u8],
    ) -> Result<IntelAliveInfo, IntelAliveError> {
        if self.state != IntelFirmwareStartState::AwaitingAlive {
            return Err(IntelAliveError::InvalidState);
        }
        if command != INTEL_UCODE_ALIVE_NTFY {
            return Err(IntelAliveError::WrongCommand);
        }
        let info = match parse_intel_alive_v3(payload) {
            Ok(info) => info,
            Err(err) => {
                self.state = IntelFirmwareStartState::Failed;
                return Err(err);
            }
        };
        if info.status != INTEL_ALIVE_STATUS_OK {
            self.state = IntelFirmwareStartState::Failed;
            return Err(IntelAliveError::FirmwareRejected(info.status));
        }
        self.state = IntelFirmwareStartState::FirmwareRunning;
        Ok(info)
    }
}

fn stage13_10t_put16(payload: &mut [u8; INTEL_ALIVE_V3_SIZE], offset: usize, value: u16) {
    payload[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn stage13_10t_put32(payload: &mut [u8; INTEL_ALIVE_V3_SIZE], offset: usize, value: u32) {
    payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn stage13_10t_valid_payload(status: u16) -> [u8; INTEL_ALIVE_V3_SIZE] {
    let mut payload = [0u8; INTEL_ALIVE_V3_SIZE];
    stage13_10t_put16(&mut payload, 0, status);
    stage13_10t_put16(&mut payload, 2, 1);
    stage13_10t_put32(&mut payload, 4, 0x1122_3344);
    stage13_10t_put32(&mut payload, 8, 0x5566_7788);
    payload[12] = 9;
    payload[13] = 0;
    payload[14] = 1;
    payload[15] = 2;
    stage13_10t_put32(&mut payload, 16, 0x0102_0304);
    stage13_10t_put32(&mut payload, 20, 0x1000_1000);
    stage13_10t_put32(&mut payload, 24, 0x1000_2000);
    stage13_10t_put32(&mut payload, 28, 0x1000_3000);
    stage13_10t_put32(&mut payload, 32, 0x1000_4000);
    stage13_10t_put32(&mut payload, 36, 0x1000_5000);
    stage13_10t_put32(&mut payload, 40, 0x1000_6000);
    stage13_10t_put32(&mut payload, 44, 0x1000_7000);
    stage13_10t_put32(&mut payload, 48, 0x800);
    stage13_10t_put32(&mut payload, 52, 0xa1a2_a3a4);
    stage13_10t_put32(&mut payload, 56, 0xb1b2_b3b4);
    stage13_10t_put32(&mut payload, 60, 0x2000_1000);
    stage13_10t_put32(&mut payload, 64, 0x2000_2000);
    payload
}

pub fn stage13_10t_self_test() -> bool {
    let ok = stage13_10t_valid_payload(INTEL_ALIVE_STATUS_OK);
    let Ok(parsed) = parse_intel_alive_v3(&ok) else {
        return false;
    };
    if parsed.status != INTEL_ALIVE_STATUS_OK
        || parsed.flags != 1
        || parsed.lmac_ucode_major != 0x1122_3344
        || parsed.lmac_ucode_minor != 0x5566_7788
        || parsed.lmac_ver_subtype != 9
        || parsed.lmac_timestamp != 0x0102_0304
        || parsed.umac_major != 0xa1a2_a3a4
        || parsed.umac_minor != 0xb1b2_b3b4
        || parsed.diagnostics.lmac_error_event_table != 0x1000_1000
        || parsed.diagnostics.lmac_scd_base != 0x1000_6000
        || parsed.diagnostics.umac_error_info != 0x2000_1000
        || parse_intel_alive_v3(&ok[..67]) != Err(IntelAliveError::UnsupportedLength)
    {
        return false;
    }

    let mut owner = Intel22000DmaContextInfo::new();
    if owner.set_rx_queue(0x100000, 0x110000, 0x120000).is_err()
        || owner
            .set_command_queue(0x130000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Lmac, &[0x31; 512])
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Umac, &[0x42; 256])
            .is_err()
    {
        return false;
    }
    let Ok(context) = owner.build_hardware_context(0x2200) else {
        return false;
    };

    let io = Stage13_10sMockIo {
        context_offset: 0,
        context_address: 0,
        context_writes: 0,
        prph_register: 0,
        prph_value: 0,
        prph_writes: 0,
        fail_prph: false,
    };
    let mut s = Intel22000FirmwareStartup::new(io);
    if s.publish_context(&context).is_err()
        || s.issue_cpu_init_run().is_err()
        || s.begin_alive_wait().is_err()
        || s.handle_alive_notification(INTEL_UCODE_ALIVE_NTFY, &ok)
            .is_err()
        || s.state() != IntelFirmwareStartState::FirmwareRunning
    {
        return false;
    }

    let io2 = Stage13_10sMockIo {
        context_offset: 0,
        context_address: 0,
        context_writes: 0,
        prph_register: 0,
        prph_value: 0,
        prph_writes: 0,
        fail_prph: false,
    };
    let mut wrong = Intel22000FirmwareStartup::new(io2);
    if wrong.publish_context(&context).is_err()
        || wrong.issue_cpu_init_run().is_err()
        || wrong.begin_alive_wait().is_err()
        || wrong.handle_alive_notification(0x7f, &ok) != Err(IntelAliveError::WrongCommand)
        || wrong.state() != IntelFirmwareStartState::AwaitingAlive
    {
        return false;
    }

    let io3 = Stage13_10sMockIo {
        context_offset: 0,
        context_address: 0,
        context_writes: 0,
        prph_register: 0,
        prph_value: 0,
        prph_writes: 0,
        fail_prph: false,
    };
    let mut rejected = Intel22000FirmwareStartup::new(io3);
    let bad = stage13_10t_valid_payload(INTEL_ALIVE_STATUS_ERR);
    rejected.publish_context(&context).is_ok()
        && rejected.issue_cpu_init_run().is_ok()
        && rejected.begin_alive_wait().is_ok()
        && rejected.handle_alive_notification(INTEL_UCODE_ALIVE_NTFY, &bad)
            == Err(IntelAliveError::FirmwareRejected(INTEL_ALIVE_STATUS_ERR))
        && rejected.state() == IntelFirmwareStartState::Failed
}

// === Stage 13.10V: version-aware Intel 22000/AX200 ALIVE ABI ===
pub const INTEL_ALIVE_V7_SIZE: usize = 144;
pub const INTEL_ALIVE_V8_SIZE: usize = 152;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelAliveAbiVersion {
    V3,
    V7,
    V8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IntelAliveImrInfo {
    pub base_address: u64,
    pub size: u32,
    pub enabled: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntelVersionedAliveInfo {
    pub abi: IntelAliveAbiVersion,
    pub primary: IntelAliveInfo,
    pub lmac2_error_event_table: Option<u32>,
    pub sku_id: [u32; 3],
    pub imr: Option<IntelAliveImrInfo>,
    pub platform_id: Option<u64>,
}

fn alive_u64(p: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes([
        *p.get(o)?,
        *p.get(o + 1)?,
        *p.get(o + 2)?,
        *p.get(o + 3)?,
        *p.get(o + 4)?,
        *p.get(o + 5)?,
        *p.get(o + 6)?,
        *p.get(o + 7)?,
    ]))
}

fn parse_alive_primary(p: &[u8], l: usize, u: usize) -> Result<IntelAliveInfo, IntelAliveError> {
    Ok(IntelAliveInfo {
        status: alive_u16(p, 0).ok_or(IntelAliveError::UnsupportedLength)?,
        flags: alive_u16(p, 2).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_ucode_major: alive_u32(p, l).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_ucode_minor: alive_u32(p, l + 4).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_ver_subtype: *p.get(l + 8).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_ver_type: *p.get(l + 9).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_mac: *p.get(l + 10).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_opt: *p.get(l + 11).ok_or(IntelAliveError::UnsupportedLength)?,
        lmac_timestamp: alive_u32(p, l + 12).ok_or(IntelAliveError::UnsupportedLength)?,
        umac_major: alive_u32(p, u).ok_or(IntelAliveError::UnsupportedLength)?,
        umac_minor: alive_u32(p, u + 4).ok_or(IntelAliveError::UnsupportedLength)?,
        diagnostics: IntelAliveDiagnostics {
            lmac_error_event_table: alive_u32(p, l + 16)
                .ok_or(IntelAliveError::UnsupportedLength)?,
            lmac_log_event_table: alive_u32(p, l + 20).ok_or(IntelAliveError::UnsupportedLength)?,
            lmac_cpu_register: alive_u32(p, l + 24).ok_or(IntelAliveError::UnsupportedLength)?,
            lmac_dbgm_config: alive_u32(p, l + 28).ok_or(IntelAliveError::UnsupportedLength)?,
            lmac_alive_counter: alive_u32(p, l + 32).ok_or(IntelAliveError::UnsupportedLength)?,
            lmac_scd_base: alive_u32(p, l + 36).ok_or(IntelAliveError::UnsupportedLength)?,
            lmac_store_forward_address: alive_u32(p, l + 40)
                .ok_or(IntelAliveError::UnsupportedLength)?,
            lmac_store_forward_size: alive_u32(p, l + 44)
                .ok_or(IntelAliveError::UnsupportedLength)?,
            umac_error_info: alive_u32(p, u + 8).ok_or(IntelAliveError::UnsupportedLength)?,
            umac_debug_print_buffer: alive_u32(p, u + 12)
                .ok_or(IntelAliveError::UnsupportedLength)?,
        },
    })
}

pub fn parse_intel_alive(
    abi: IntelAliveAbiVersion,
    p: &[u8],
) -> Result<IntelVersionedAliveInfo, IntelAliveError> {
    if abi == IntelAliveAbiVersion::V3 {
        return Ok(IntelVersionedAliveInfo {
            abi,
            primary: parse_intel_alive_v3(p)?,
            lmac2_error_event_table: None,
            sku_id: [0; 3],
            imr: None,
            platform_id: None,
        });
    }
    let n = if abi == IntelAliveAbiVersion::V7 {
        INTEL_ALIVE_V7_SIZE
    } else {
        INTEL_ALIVE_V8_SIZE
    };
    if p.len() != n {
        return Err(IntelAliveError::UnsupportedLength);
    }
    Ok(IntelVersionedAliveInfo {
        abi,
        primary: parse_alive_primary(p, 4, 100)?,
        lmac2_error_event_table: Some(alive_u32(p, 68).ok_or(IntelAliveError::UnsupportedLength)?),
        sku_id: [
            alive_u32(p, 116).ok_or(IntelAliveError::UnsupportedLength)?,
            alive_u32(p, 120).ok_or(IntelAliveError::UnsupportedLength)?,
            alive_u32(p, 124).ok_or(IntelAliveError::UnsupportedLength)?,
        ],
        imr: Some(IntelAliveImrInfo {
            base_address: alive_u64(p, 128).ok_or(IntelAliveError::UnsupportedLength)?,
            size: alive_u32(p, 136).ok_or(IntelAliveError::UnsupportedLength)?,
            enabled: alive_u32(p, 140).ok_or(IntelAliveError::UnsupportedLength)?,
        }),
        platform_id: if abi == IntelAliveAbiVersion::V8 {
            Some(alive_u64(p, 144).ok_or(IntelAliveError::UnsupportedLength)?)
        } else {
            None
        },
    })
}

impl<I: IntelFirmwareStartupIo> Intel22000FirmwareStartup<I> {
    pub fn handle_alive_notification_versioned(
        &mut self,
        command: u8,
        abi: IntelAliveAbiVersion,
        p: &[u8],
    ) -> Result<IntelVersionedAliveInfo, IntelAliveError> {
        if self.state != IntelFirmwareStartState::AwaitingAlive {
            return Err(IntelAliveError::InvalidState);
        }
        if command != INTEL_UCODE_ALIVE_NTFY {
            return Err(IntelAliveError::WrongCommand);
        }
        let info = match parse_intel_alive(abi, p) {
            Ok(v) => v,
            Err(e) => {
                self.state = IntelFirmwareStartState::Failed;
                return Err(e);
            }
        };
        if info.primary.status != INTEL_ALIVE_STATUS_OK {
            self.state = IntelFirmwareStartState::Failed;
            return Err(IntelAliveError::FirmwareRejected(info.primary.status));
        }
        self.state = IntelFirmwareStartState::FirmwareRunning;
        Ok(info)
    }
}

fn stage13_10v_put16(p: &mut [u8], o: usize, v: u16) {
    p[o..o + 2].copy_from_slice(&v.to_le_bytes());
}
fn stage13_10v_put32(p: &mut [u8], o: usize, v: u32) {
    p[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn stage13_10v_put64(p: &mut [u8], o: usize, v: u64) {
    p[o..o + 8].copy_from_slice(&v.to_le_bytes());
}
fn stage13_10v_fill(p: &mut [u8]) {
    stage13_10v_put16(p, 0, INTEL_ALIVE_STATUS_OK);
    stage13_10v_put16(p, 2, 1);
    stage13_10v_put32(p, 4, 0x11110001);
    stage13_10v_put32(p, 8, 0x11110002);
    p[12] = 9;
    p[14] = 1;
    p[15] = 2;
    stage13_10v_put32(p, 16, 0x11110003);
    stage13_10v_put32(p, 20, 0x11111000);
    stage13_10v_put32(p, 40, 0x11116000);
    stage13_10v_put32(p, 52, 0x22220001);
    stage13_10v_put32(p, 68, 0x22221000);
    stage13_10v_put32(p, 100, 0x33330001);
    stage13_10v_put32(p, 104, 0x33330002);
    stage13_10v_put32(p, 108, 0x33331000);
    stage13_10v_put32(p, 112, 0x33332000);
    stage13_10v_put32(p, 116, 0x44440001);
    stage13_10v_put32(p, 120, 0x44440002);
    stage13_10v_put32(p, 124, 0x44440003);
    stage13_10v_put64(p, 128, 0x0000000180000000);
    stage13_10v_put32(p, 136, 0x00200000);
    stage13_10v_put32(p, 140, 1);
}
pub fn stage13_10v_self_test() -> bool {
    let mut v7 = [0u8; INTEL_ALIVE_V7_SIZE];
    stage13_10v_fill(&mut v7);
    let Ok(a7) = parse_intel_alive(IntelAliveAbiVersion::V7, &v7) else {
        return false;
    };
    if a7.primary.lmac_ucode_major != 0x11110001
        || a7.primary.diagnostics.lmac_error_event_table != 0x11111000
        || a7.primary.umac_major != 0x33330001
        || a7.lmac2_error_event_table != Some(0x22221000)
        || a7.sku_id != [0x44440001, 0x44440002, 0x44440003]
        || a7.imr
            != Some(IntelAliveImrInfo {
                base_address: 0x0000000180000000,
                size: 0x00200000,
                enabled: 1,
            })
        || a7.platform_id.is_some()
        || parse_intel_alive(IntelAliveAbiVersion::V7, &v7[..143])
            != Err(IntelAliveError::UnsupportedLength)
    {
        return false;
    }
    let mut v8 = [0u8; INTEL_ALIVE_V8_SIZE];
    stage13_10v_fill(&mut v8);
    stage13_10v_put64(&mut v8, 144, 0x8877665544332211);
    let Ok(a8) = parse_intel_alive(IntelAliveAbiVersion::V8, &v8) else {
        return false;
    };
    if a8.platform_id != Some(0x8877665544332211)
        || parse_intel_alive(IntelAliveAbiVersion::V8, &v8[..151])
            != Err(IntelAliveError::UnsupportedLength)
    {
        return false;
    }
    let v3 = stage13_10t_valid_payload(INTEL_ALIVE_STATUS_OK);
    if parse_intel_alive(IntelAliveAbiVersion::V3, &v3).is_err() {
        return false;
    }
    let mut owner = Intel22000DmaContextInfo::new();
    if owner.set_rx_queue(0x100000, 0x110000, 0x120000).is_err()
        || owner
            .set_command_queue(0x130000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Lmac, &[0x31; 512])
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Umac, &[0x42; 256])
            .is_err()
    {
        return false;
    }
    let Ok(context) = owner.build_hardware_context(0x2200) else {
        return false;
    };
    let io = Stage13_10sMockIo {
        context_offset: 0,
        context_address: 0,
        context_writes: 0,
        prph_register: 0,
        prph_value: 0,
        prph_writes: 0,
        fail_prph: false,
    };
    let mut s = Intel22000FirmwareStartup::new(io);
    s.publish_context(&context).is_ok()
        && s.issue_cpu_init_run().is_ok()
        && s.begin_alive_wait().is_ok()
        && s.handle_alive_notification_versioned(
            INTEL_UCODE_ALIVE_NTFY,
            IntelAliveAbiVersion::V8,
            &v8,
        )
        .is_ok()
        && s.state() == IntelFirmwareStartState::FirmwareRunning
}

// === Stage 13.10W: Intel RX notification packet delivery ===
pub const INTEL_RX_PACKET_PREFIX_SIZE: usize = 4;
pub const INTEL_RX_COMMAND_HEADER_SIZE: usize = 4;
pub const INTEL_RX_PACKET_HEADER_SIZE: usize = 8;
pub const INTEL_RX_FRAME_SIZE_MASK: u32 = 0x0000_3fff;
/// AX201 uses the new RX MPDU descriptor layout. The descriptor contains
/// DW2..DW17; the 802.11 MPDU follows it in the same notification payload.
pub const AX201_RX_MPDU_DESC_BYTES: usize = 64;
pub const AX201_RX_MPDU_CMD: u8 = 0xc1;
pub const AX201_RX_MPDU_STATUS_CRC_OK: u32 = 1 << 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntelRxNotification<'a> {
    pub command: u8,
    pub group_id: u8,
    pub sequence: u16,
    pub payload: &'a [u8],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelRxNotificationError {
    TooShort,
    InvalidLength,
    Truncated,
    TrailingBytes,
    Alive(IntelAliveError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ax201RxMpdu<'a> {
    pub frame: &'a [u8],
    pub signal_dbm: i8,
    pub channel: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ax201RxMpduError {
    WrongCommand,
    TruncatedDescriptor,
    InvalidStatus,
    InvalidLength,
    TruncatedFrame,
}

/// Decode an AX201 `REPLY_RX_MPDU_CMD` payload without borrowing device-owned
/// memory beyond the supplied RX buffer. The new descriptor's MPDU length is
/// authoritative; trailing bytes are rejected by the common notification
/// parser before this function is called.
pub fn parse_ax201_rx_mpdu(
    notification: IntelRxNotification<'_>,
) -> Result<Ax201RxMpdu<'_>, Ax201RxMpduError> {
    if notification.group_id != INTEL_LEGACY_GROUP || notification.command != AX201_RX_MPDU_CMD {
        return Err(Ax201RxMpduError::WrongCommand);
    }
    let payload = notification.payload;
    if payload.len() < AX201_RX_MPDU_DESC_BYTES {
        return Err(Ax201RxMpduError::TruncatedDescriptor);
    }
    let mpdu_len = usize::from(u16::from_le_bytes([payload[0], payload[1]]));
    let status = u32::from_le_bytes(payload[12..16].try_into().unwrap());
    if status & AX201_RX_MPDU_STATUS_CRC_OK == 0 {
        return Err(Ax201RxMpduError::InvalidStatus);
    }
    if mpdu_len < crate::wifi80211::MGMT_HEADER_LEN {
        return Err(Ax201RxMpduError::InvalidLength);
    }
    let frame_end = AX201_RX_MPDU_DESC_BYTES
        .checked_add(mpdu_len)
        .ok_or(Ax201RxMpduError::InvalidLength)?;
    if frame_end > payload.len() {
        return Err(Ax201RxMpduError::TruncatedFrame);
    }
    // The v3 descriptor stores energy A/B at DW12 bytes 40/41 and the
    // channel at byte 42. Firmware energy is a positive magnitude; clamp the
    // conversion to the i8 RSSI range until calibration is available.
    let energy = u16::from(payload[40]).saturating_add(u16::from(payload[41])) / 2;
    let signal_dbm = -(i16::try_from(energy.min(127)).unwrap_or(127) as i8);
    Ok(Ax201RxMpdu {
        frame: &payload[AX201_RX_MPDU_DESC_BYTES..frame_end],
        signal_dbm,
        channel: payload[42],
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelRxDispatchResult {
    Ignored {
        command: u8,
        group_id: u8,
        sequence: u16,
    },
    Alive(IntelVersionedAliveInfo),
}

pub fn parse_intel_rx_notification(
    bytes: &[u8],
) -> Result<IntelRxNotification<'_>, IntelRxNotificationError> {
    if bytes.len() < INTEL_RX_PACKET_HEADER_SIZE {
        return Err(IntelRxNotificationError::TooShort);
    }
    let len_n_flags = alive_u32(bytes, 0).ok_or(IntelRxNotificationError::TooShort)?;
    let packet_len = (len_n_flags & INTEL_RX_FRAME_SIZE_MASK) as usize;
    if packet_len < INTEL_RX_COMMAND_HEADER_SIZE {
        return Err(IntelRxNotificationError::InvalidLength);
    }
    let total_len = INTEL_RX_PACKET_PREFIX_SIZE
        .checked_add(packet_len)
        .ok_or(IntelRxNotificationError::InvalidLength)?;
    if total_len > bytes.len() {
        return Err(IntelRxNotificationError::Truncated);
    }
    if total_len != bytes.len() {
        return Err(IntelRxNotificationError::TrailingBytes);
    }
    Ok(IntelRxNotification {
        command: bytes[4],
        group_id: bytes[5],
        sequence: u16::from_le_bytes([bytes[6], bytes[7]]),
        payload: &bytes[8..total_len],
    })
}

pub fn parse_ax201_scan_complete(
    notification: IntelRxNotification<'_>,
) -> Result<Ax201ScanComplete, Ax201ScanError> {
    if notification.group_id != INTEL_SCAN_GROUP
        || notification.command != INTEL_SCAN_COMPLETE_UMAC_CMD
    {
        return Err(Ax201ScanError::WrongCommand);
    }
    if notification.payload.len() != 16 {
        return Err(Ax201ScanError::InvalidPayload);
    }
    Ok(Ax201ScanComplete {
        uid: u32::from_le_bytes(notification.payload[0..4].try_into().unwrap()),
        last_schedule: notification.payload[4],
        last_iteration: notification.payload[5],
        status: notification.payload[6],
        ebs_status: notification.payload[7],
        time_from_last_iteration: u32::from_le_bytes(
            notification.payload[8..12].try_into().unwrap(),
        ),
    })
}

impl<I: IntelFirmwareStartupIo> Intel22000FirmwareStartup<I> {
    pub fn dispatch_rx_notification(
        &mut self,
        abi: IntelAliveAbiVersion,
        bytes: &[u8],
    ) -> Result<IntelRxDispatchResult, IntelRxNotificationError> {
        let packet = parse_intel_rx_notification(bytes)?;
        if packet.command != INTEL_UCODE_ALIVE_NTFY {
            return Ok(IntelRxDispatchResult::Ignored {
                command: packet.command,
                group_id: packet.group_id,
                sequence: packet.sequence,
            });
        }
        self.handle_alive_notification_versioned(packet.command, abi, packet.payload)
            .map(IntelRxDispatchResult::Alive)
            .map_err(IntelRxNotificationError::Alive)
    }
}
fn stage13_10w_packet_into<'a>(
    storage: &'a mut [u8],
    command: u8,
    group: u8,
    sequence: u16,
    payload: &[u8],
) -> Option<&'a [u8]> {
    let packet_len = INTEL_RX_COMMAND_HEADER_SIZE.checked_add(payload.len())?;
    let total_len = INTEL_RX_PACKET_PREFIX_SIZE.checked_add(packet_len)?;
    if packet_len > INTEL_RX_FRAME_SIZE_MASK as usize || total_len > storage.len() {
        return None;
    }
    storage[..total_len].fill(0);
    storage[0..4].copy_from_slice(&(packet_len as u32).to_le_bytes());
    storage[4] = command;
    storage[5] = group;
    storage[6..8].copy_from_slice(&sequence.to_le_bytes());
    storage[8..total_len].copy_from_slice(payload);
    Some(&storage[..total_len])
}
fn stage13_10w_startup(
    context: &Intel22000HardwareContextInfo,
) -> Option<Intel22000FirmwareStartup<Stage13_10sMockIo>> {
    let io = Stage13_10sMockIo {
        context_offset: 0,
        context_address: 0,
        context_writes: 0,
        prph_register: 0,
        prph_value: 0,
        prph_writes: 0,
        fail_prph: false,
    };
    let mut s = Intel22000FirmwareStartup::new(io);
    if s.publish_context(context).is_err()
        || s.issue_cpu_init_run().is_err()
        || s.begin_alive_wait().is_err()
    {
        return None;
    }
    Some(s)
}
pub fn stage13_10w_self_test() -> bool {
    let mut owner = Intel22000DmaContextInfo::new();
    if owner.set_rx_queue(0x100000, 0x110000, 0x120000).is_err()
        || owner
            .set_command_queue(0x130000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Lmac, &[0x31; 512])
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Umac, &[0x42; 256])
            .is_err()
    {
        return false;
    }
    let Ok(context) = owner.build_hardware_context(0x2200) else {
        return false;
    };
    let mut v8 = [0u8; INTEL_ALIVE_V8_SIZE];
    stage13_10v_fill(&mut v8);
    stage13_10v_put64(&mut v8, 144, 0x8877665544332211);
    let mut packet_storage = [0u8; INTEL_RX_PACKET_HEADER_SIZE + INTEL_ALIVE_V8_SIZE];
    let Some(packet) =
        stage13_10w_packet_into(&mut packet_storage, INTEL_UCODE_ALIVE_NTFY, 0, 0x1234, &v8)
    else {
        return false;
    };
    let Ok(p) = parse_intel_rx_notification(packet) else {
        return false;
    };
    if p.command != INTEL_UCODE_ALIVE_NTFY
        || p.group_id != 0
        || p.sequence != 0x1234
        || p.payload != v8
    {
        return false;
    }
    if parse_intel_rx_notification(&packet[..7]) != Err(IntelRxNotificationError::TooShort) {
        return false;
    }
    // Invalid-length case: fixed storage, no heap allocation.
    let mut bad = [0u8; INTEL_RX_PACKET_HEADER_SIZE + INTEL_ALIVE_V8_SIZE];
    bad.copy_from_slice(packet);
    bad[0..4].copy_from_slice(&3u32.to_le_bytes());
    if parse_intel_rx_notification(&bad) != Err(IntelRxNotificationError::InvalidLength) {
        return false;
    }

    // Truncation case: claim one byte more than the buffer contains.
    let mut trunc = [0u8; INTEL_RX_PACKET_HEADER_SIZE + INTEL_ALIVE_V8_SIZE];
    trunc.copy_from_slice(packet);
    trunc[0..4].copy_from_slice(&((4 + v8.len() + 1) as u32).to_le_bytes());
    if parse_intel_rx_notification(&trunc) != Err(IntelRxNotificationError::Truncated) {
        return false;
    }

    // Trailing-byte case: provide one byte beyond the declared packet.
    let mut trail = [0u8; INTEL_RX_PACKET_HEADER_SIZE + INTEL_ALIVE_V8_SIZE + 1];
    trail[..packet.len()].copy_from_slice(packet);
    trail[packet.len()] = 0;
    if parse_intel_rx_notification(&trail) != Err(IntelRxNotificationError::TrailingBytes) {
        return false;
    }
    let mut rx_payload = [0u8; AX201_RX_MPDU_DESC_BYTES + 24];
    rx_payload[0..2].copy_from_slice(&24u16.to_le_bytes());
    rx_payload[12..16].copy_from_slice(&AX201_RX_MPDU_STATUS_CRC_OK.to_le_bytes());
    rx_payload[40] = 48;
    rx_payload[41] = 52;
    rx_payload[42] = 6;
    rx_payload[AX201_RX_MPDU_DESC_BYTES] = 0x80;
    let mut rx_storage = [0u8; INTEL_RX_PACKET_HEADER_SIZE + AX201_RX_MPDU_DESC_BYTES + 24];
    let Some(rx_packet) = stage13_10w_packet_into(
        &mut rx_storage,
        AX201_RX_MPDU_CMD,
        INTEL_LEGACY_GROUP,
        0x8000,
        &rx_payload,
    ) else {
        return false;
    };
    let Ok(rx_notification) = parse_intel_rx_notification(rx_packet) else {
        return false;
    };
    let Ok(rx) = parse_ax201_rx_mpdu(rx_notification) else {
        return false;
    };
    if rx.frame.len() != 24 || rx.frame[0] != 0x80 || rx.channel != 6 || rx.signal_dbm != -50 {
        return false;
    }
    let mut bad_rx = rx_payload;
    bad_rx[12..16].fill(0);
    let mut bad_rx_storage = [0u8; INTEL_RX_PACKET_HEADER_SIZE + AX201_RX_MPDU_DESC_BYTES + 24];
    let Some(bad_rx_packet) = stage13_10w_packet_into(
        &mut bad_rx_storage,
        AX201_RX_MPDU_CMD,
        INTEL_LEGACY_GROUP,
        0x8000,
        &bad_rx,
    ) else {
        return false;
    };
    let Ok(bad_rx_notification) = parse_intel_rx_notification(bad_rx_packet) else {
        return false;
    };
    if parse_ax201_rx_mpdu(bad_rx_notification) != Err(Ax201RxMpduError::InvalidStatus) {
        return false;
    }
    let mut unrelated_storage = [0u8; INTEL_RX_PACKET_HEADER_SIZE + 4];
    let Some(unrelated) =
        stage13_10w_packet_into(&mut unrelated_storage, 0x7f, 3, 9, &[1, 2, 3, 4])
    else {
        return false;
    };
    let Some(mut ignored) = stage13_10w_startup(&context) else {
        return false;
    };
    if ignored.dispatch_rx_notification(IntelAliveAbiVersion::V8, unrelated)
        != Ok(IntelRxDispatchResult::Ignored {
            command: 0x7f,
            group_id: 3,
            sequence: 9,
        })
        || ignored.state() != IntelFirmwareStartState::AwaitingAlive
    {
        return false;
    }
    let Some(mut startup) = stage13_10w_startup(&context) else {
        return false;
    };
    let Ok(IntelRxDispatchResult::Alive(alive)) =
        startup.dispatch_rx_notification(IntelAliveAbiVersion::V8, packet)
    else {
        return false;
    };
    if alive.primary.status != INTEL_ALIVE_STATUS_OK
        || alive.platform_id != Some(0x8877665544332211)
        || startup.state() != IntelFirmwareStartState::FirmwareRunning
    {
        return false;
    }
    let mut rejected_payload = v8;
    stage13_10v_put16(&mut rejected_payload, 0, INTEL_ALIVE_STATUS_ERR);
    let mut rejected_storage = [0u8; INTEL_RX_PACKET_HEADER_SIZE + INTEL_ALIVE_V8_SIZE];
    let Some(rejected_packet) = stage13_10w_packet_into(
        &mut rejected_storage,
        INTEL_UCODE_ALIVE_NTFY,
        0,
        0x4321,
        &rejected_payload,
    ) else {
        return false;
    };
    let Some(mut rejected) = stage13_10w_startup(&context) else {
        return false;
    };
    rejected.dispatch_rx_notification(IntelAliveAbiVersion::V8, rejected_packet)
        == Err(IntelRxNotificationError::Alive(
            IntelAliveError::FirmwareRejected(INTEL_ALIVE_STATUS_ERR),
        ))
        && rejected.state() == IntelFirmwareStartState::Failed
}

// === Stage 13.10X: Intel RX DMA completion ownership boundary ===
pub const INTEL_RX_DMA_BUFFER_SIZE: u16 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntelRxDmaError {
    Dma,
    Queue(crate::wifi_hw::QueueError),
    InvalidCompletionLength,
    Notification(IntelRxNotificationError),
}

pub struct IntelRxDmaQueue {
    queue: crate::wifi_hw::OwnedBufferQueue,
    completed_lengths: [u16; crate::wifi_hw::DMA_RING_CAPACITY],
}

impl IntelRxDmaQueue {
    pub const fn new() -> Self {
        Self {
            queue: crate::wifi_hw::OwnedBufferQueue::new(),
            completed_lengths: [0; crate::wifi_hw::DMA_RING_CAPACITY],
        }
    }

    pub fn post_buffer(
        &mut self,
    ) -> Result<(usize, crate::wifi_hw::DmaDescriptor), IntelRxDmaError> {
        let buffer = crate::wifi_hw::OwnedDmaBuffer::allocate(INTEL_RX_DMA_BUFFER_SIZE, 0)
            .map_err(|_| IntelRxDmaError::Dma)?;
        self.queue.submit(buffer).map_err(IntelRxDmaError::Queue)
    }

    /// Stage X models the device DMA producer. A later physical interrupt/RX
    /// stage will replace this producer while preserving the ownership rules.
    pub fn complete_from_device(
        &mut self,
        slot: usize,
        bytes: &[u8],
    ) -> Result<(), IntelRxDmaError> {
        if bytes.is_empty() || bytes.len() > usize::from(INTEL_RX_DMA_BUFFER_SIZE) {
            return Err(IntelRxDmaError::InvalidCompletionLength);
        }
        self.queue
            .device_write(slot, bytes)
            .map_err(IntelRxDmaError::Queue)?;
        self.completed_lengths[slot] =
            u16::try_from(bytes.len()).map_err(|_| IntelRxDmaError::InvalidCompletionLength)?;
        self.queue.complete(slot).map_err(IntelRxDmaError::Queue)
    }

    pub fn dispatch_completed<I: IntelFirmwareStartupIo>(
        &mut self,
        startup: &mut Intel22000FirmwareStartup<I>,
        abi: IntelAliveAbiVersion,
    ) -> Result<IntelRxDispatchResult, IntelRxDmaError> {
        let (slot, buffer) = self
            .queue
            .reclaim_with_slot()
            .map_err(IntelRxDmaError::Queue)?;
        let length = usize::from(self.completed_lengths[slot]);
        self.completed_lengths[slot] = 0;
        if length == 0 || length > usize::from(buffer.length()) {
            return Err(IntelRxDmaError::InvalidCompletionLength);
        }

        let mut packet = [0u8; INTEL_RX_DMA_BUFFER_SIZE as usize];
        buffer
            .read_bytes(0, &mut packet[..length])
            .map_err(|_| IntelRxDmaError::Dma)?;
        startup
            .dispatch_rx_notification(abi, &packet[..length])
            .map_err(IntelRxDmaError::Notification)
    }

    pub const fn device_owned(&self) -> usize {
        self.queue.device_owned()
    }

    pub const fn completed(&self) -> usize {
        self.queue.completed()
    }
}

pub fn stage13_10x_self_test() -> bool {
    let mut owner = Intel22000DmaContextInfo::new();
    if owner.set_rx_queue(0x100000, 0x110000, 0x120000).is_err()
        || owner
            .set_command_queue(0x130000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Lmac, &[0x31; 512])
            .is_err()
    {
        return false;
    }
    let Ok(context) = owner.build_hardware_context(0x2200) else {
        return false;
    };
    let Some(mut startup) = stage13_10w_startup(&context) else {
        return false;
    };

    let mut v8 = [0u8; INTEL_ALIVE_V8_SIZE];
    stage13_10v_fill(&mut v8);
    stage13_10v_put64(&mut v8, 144, 0x8877665544332211);
    let mut packet_storage = [0u8; INTEL_RX_PACKET_HEADER_SIZE + INTEL_ALIVE_V8_SIZE];
    let Some(packet) =
        stage13_10w_packet_into(&mut packet_storage, INTEL_UCODE_ALIVE_NTFY, 0, 0x55aa, &v8)
    else {
        return false;
    };

    let mut rx = IntelRxDmaQueue::new();
    let Ok((slot, descriptor)) = rx.post_buffer() else {
        return false;
    };
    if descriptor.length != INTEL_RX_DMA_BUFFER_SIZE
        || rx.device_owned() != 1
        || rx.completed() != 0
        || rx.complete_from_device(slot, &[]).is_ok()
    {
        return false;
    }

    if rx.complete_from_device(slot, packet).is_err()
        || rx.device_owned() != 0
        || rx.completed() != 1
    {
        return false;
    }

    let Ok(IntelRxDispatchResult::Alive(alive)) =
        rx.dispatch_completed(&mut startup, IntelAliveAbiVersion::V8)
    else {
        return false;
    };
    if alive.primary.status != INTEL_ALIVE_STATUS_OK
        || alive.platform_id != Some(0x8877665544332211)
        || startup.state() != IntelFirmwareStartState::FirmwareRunning
        || rx.completed() != 0
    {
        return false;
    }

    // A reclaimed slot cannot be completed again until it is reposted.
    rx.complete_from_device(slot, packet)
        == Err(IntelRxDmaError::Queue(
            crate::wifi_hw::QueueError::NotDeviceOwned,
        ))
}
pub fn stage13_10s_self_test() -> bool {
    let mut owner = Intel22000DmaContextInfo::new();
    if owner.set_rx_queue(0x100000, 0x110000, 0x120000).is_err()
        || owner
            .set_command_queue(0x130000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Lmac, &[0x31; 512])
            .is_err()
        || owner
            .stage_firmware_chunk(IntelContextInfoImageKind::Umac, &[0x42; 256])
            .is_err()
    {
        return false;
    }
    let Ok(context) = owner.build_hardware_context(0x2200) else {
        return false;
    };
    let io = Stage13_10sMockIo {
        context_offset: 0,
        context_address: 0,
        context_writes: 0,
        prph_register: 0,
        prph_value: 0,
        prph_writes: 0,
        fail_prph: false,
    };
    let mut s = Intel22000FirmwareStartup::new(io);
    if s.issue_cpu_init_run() != Err(IntelFirmwareStartError::InvalidState)
        || s.publish_context(&context).is_err()
        || s.issue_cpu_init_run().is_err()
        || s.begin_alive_wait().is_err()
        || s.state() != IntelFirmwareStartState::AwaitingAlive
    {
        return false;
    }
    let io = s.into_inner();
    if io.context_writes != 1
        || io.context_offset != INTEL_CSR_CONTEXT_INFO_BASE
        || io.context_address != context.physical_address()
        || io.prph_writes != 1
        || io.prph_register != INTEL_UREG_CPU_INIT_RUN
        || io.prph_value != 1
    {
        return false;
    }
    let fio = Stage13_10sMockIo {
        context_offset: 0,
        context_address: 0,
        context_writes: 0,
        prph_register: 0,
        prph_value: 0,
        prph_writes: 0,
        fail_prph: true,
    };
    let mut f = Intel22000FirmwareStartup::new(fio);
    f.publish_context(&context).is_ok()
        && f.issue_cpu_init_run() == Err(IntelFirmwareStartError::Prph)
        && f.state() == IntelFirmwareStartState::Failed
}

pub fn stage13_10r_self_test() -> bool {
    let mut c = Intel22000DmaContextInfo::new();
    if c.set_rx_queue(0x100000, 0x110000, 0x120000).is_err()
        || c.set_command_queue(0x130000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || c.stage_firmware_chunk(IntelContextInfoImageKind::Lmac, &[0x11; 256])
            .is_err()
        || c.stage_firmware_chunk(IntelContextInfoImageKind::Umac, &[0x22; 128])
            .is_err()
    {
        return false;
    }
    let Ok(h) = c.build_hardware_context(0x1234) else {
        return false;
    };
    let r16 = |o| Some(u16::from_le_bytes([h.byte(o).ok()?, h.byte(o + 1).ok()?]));
    let r64 = |o| {
        let mut b = [0; 8];
        for (i, v) in b.iter_mut().enumerate() {
            *v = h.byte(o + i).ok()?;
        }
        Some(u64::from_le_bytes(b))
    };
    if r16(0) != Some(0x1234)
        || r16(4) != Some(INTEL_CONTEXT_INFO_WIRE_DWORDS)
        || r64(24) != Some(0x100000)
        || r64(48) != Some(0x130000)
        || h.byte(56).ok() != Some(INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
    {
        return false;
    }
    let mut io = Stage13_10rMockIo {
        offset: 0,
        address: 0,
        writes: 0,
    };
    publish_hardware_context(&h, &mut io).is_ok()
        && io.offset == 0x40
        && io.address == h.physical_address()
        && io.address != 0
        && io.writes == 1
}

pub fn stage13_10q_self_test() -> bool {
    if !ax200_runtime_copy_self_test() {
        return false;
    }
    let mut context = Intel22000DmaContextInfo::new();
    if context
        .set_rx_queue(0x0010_0000, 0x0011_0000, 0x0012_0000)
        .is_err()
        || context
            .set_command_queue(0x0013_0000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || context.ready_for_publication()
    {
        return false;
    }

    let mut lmac = [0u8; 4096];
    for (index, byte) in lmac.iter_mut().enumerate() {
        *byte = (index & 0xff) as u8;
    }
    let umac = [0xa5u8; 1024];
    let Ok(lmac_entry) = context.stage_firmware_chunk(IntelContextInfoImageKind::Lmac, &lmac)
    else {
        return false;
    };
    let Ok(umac_entry) = context.stage_firmware_chunk(IntelContextInfoImageKind::Umac, &umac)
    else {
        return false;
    };

    if lmac_entry.physical_address == 0
        || umac_entry.physical_address == 0
        || lmac_entry.physical_address == umac_entry.physical_address
        || lmac_entry.length != 4096
        || umac_entry.length != 1024
        || context.firmware_buffer_count() != 2
        || context.manifest().lmac_count() != 1
        || context.manifest().umac_count() != 1
        || context.manifest().entry(IntelContextInfoImageKind::Lmac, 0) != Some(lmac_entry)
        || context.manifest().entry(IntelContextInfoImageKind::Umac, 0) != Some(umac_entry)
        || context.staged_byte(0, 0) != Ok(lmac[0])
        || context.staged_byte(0, 4095) != Ok(lmac[4095])
        || context.staged_byte(1, 0) != Ok(0xa5)
        || !context.ready_for_publication()
    {
        return false;
    }

    let large = [0x6cu8; INTEL_CONTEXT_INFO_MAX_DRAM_CHUNK_SIZE];
    let Ok(large_entry) = context.stage_firmware_chunk(IntelContextInfoImageKind::Paging, &large)
    else {
        return false;
    };
    if large_entry.length != INTEL_CONTEXT_INFO_MAX_DRAM_CHUNK_SIZE as u32
        || context.staged_byte(2, 0) != Ok(0x6c)
        || context.staged_byte(2, INTEL_CONTEXT_INFO_MAX_DRAM_CHUNK_SIZE - 1) != Ok(0x6c)
    {
        return false;
    }

    let too_large = [0u8; INTEL_CONTEXT_INFO_MAX_DRAM_CHUNK_SIZE + 1];
    context.stage_firmware_chunk(IntelContextInfoImageKind::Paging, &too_large)
        == Err(IntelContextInfoBuildError::Manifest(
            IntelContextInfoError::SectionTooLarge,
        ))
        && context.firmware_buffer_count() == 3
        && context.manifest().paging_count() == 1
}

fn ax200_runtime_copy_self_test() -> bool {
    let before = crate::memory::stats().allocated_frames;
    // Small real-format runtime layout: three payloads and two separators.
    // Mutating the source after the constructor returns proves it is not
    // retained as the asynchronous DMA backing store.
    let mut bytes = [0u8; INTEL_TLV_UCODE_HEADER_SIZE + 5 * 16 + 12];
    bytes[4..8].copy_from_slice(&INTEL_TLV_UCODE_MAGIC.to_le_bytes());
    for (index, offset) in [
        0x1000u32,
        INTEL_CPU_SEPARATOR,
        0x2000,
        INTEL_PAGING_SEPARATOR,
        0x3000,
    ]
    .into_iter()
    .enumerate()
    {
        let start = INTEL_TLV_UCODE_HEADER_SIZE + index * 16;
        bytes[start..start + 4].copy_from_slice(&INTEL_TLV_SEC_RT.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&8u32.to_le_bytes());
        bytes[start + 8..start + 12].copy_from_slice(&offset.to_le_bytes());
        if matches!(offset, INTEL_CPU_SEPARATOR | INTEL_PAGING_SEPARATOR) {
            bytes[start + 12..start + 16].fill(0);
        } else {
            bytes[start + 12..start + 16].fill(index as u8 + 1);
        }
    }
    let start = INTEL_TLV_UCODE_HEADER_SIZE + 5 * 16;
    bytes[start..start + 4].copy_from_slice(&INTEL_TLV_PAGING.to_le_bytes());
    bytes[start + 4..start + 8].copy_from_slice(&4u32.to_le_bytes());
    bytes[start + 8..start + 12].copy_from_slice(&4096u32.to_le_bytes());
    let Ok(firmware) = IntelTlvFirmware::parse(&bytes) else {
        return false;
    };
    let Ok(context) = Intel22000DmaContextInfo::from_runtime_firmware(&firmware) else {
        return false;
    };
    bytes.fill(0);
    let valid = context.firmware_buffer_count() == 3
        && context.manifest().lmac_count() == 1
        && context.manifest().umac_count() == 1
        && context.manifest().paging_count() == 1
        && context.staged_byte(0, 3) == Ok(1)
        && context.staged_byte(1, 3) == Ok(3)
        && context.staged_byte(2, 3) == Ok(5)
        && !context.ready_for_publication()
        && crate::memory::stats().allocated_frames == before + 3;
    drop(context);
    valid && crate::memory::stats().allocated_frames == before
}

pub fn stage13_10ae_ax201_firmware_prepare_self_test() -> bool {
    let Some(function) = crate::wifi_hw::stage13_10i_ax201_supported_function() else {
        return false;
    };

    // A bounded, canonical 22000 container: LMAC, CPU separator, UMAC,
    // paging separator, paging payload, plus paging-size metadata. The API
    // field is 77, matching the AX201 QuZ profile.
    let mut bytes = [0u8; INTEL_TLV_UCODE_HEADER_SIZE + 5 * 16 + 12];
    bytes[4..8].copy_from_slice(&INTEL_TLV_UCODE_MAGIC.to_le_bytes());
    bytes[72..76].copy_from_slice(&0x0000_4d00u32.to_le_bytes());
    for (index, offset) in [
        0x1000u32,
        INTEL_CPU_SEPARATOR,
        0x2000,
        INTEL_PAGING_SEPARATOR,
        0x3000,
    ]
    .into_iter()
    .enumerate()
    {
        let start = INTEL_TLV_UCODE_HEADER_SIZE + index * 16;
        bytes[start..start + 4].copy_from_slice(&INTEL_TLV_SEC_RT.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&8u32.to_le_bytes());
        bytes[start + 8..start + 12].copy_from_slice(&offset.to_le_bytes());
        if matches!(offset, INTEL_CPU_SEPARATOR | INTEL_PAGING_SEPARATOR) {
            bytes[start + 12..start + 16].fill(0);
        } else {
            bytes[start + 12..start + 16].fill(index as u8 + 1);
        }
    }
    let paging = INTEL_TLV_UCODE_HEADER_SIZE + 5 * 16;
    bytes[paging..paging + 4].copy_from_slice(&INTEL_TLV_PAGING.to_le_bytes());
    bytes[paging + 4..paging + 8].copy_from_slice(&4u32.to_le_bytes());
    bytes[paging + 8..paging + 12].copy_from_slice(&4096u32.to_le_bytes());

    let Ok(context) = prepare_ax201_runtime_firmware(function, &bytes) else {
        return false;
    };
    if context.firmware_buffer_count() != 3
        || context.manifest().lmac_count() != 1
        || context.manifest().umac_count() != 1
        || context.manifest().paging_count() != 1
        || context.staged_byte(0, 3) != Ok(1)
        || context.staged_byte(1, 3) != Ok(3)
        || context.staged_byte(2, 3) != Ok(5)
    {
        return false;
    }

    let mut wrong_api = bytes;
    wrong_api[72..76].copy_from_slice(&0x0000_4c00u32.to_le_bytes());
    let Ok(parsed) = IntelTlvFirmware::parse(&wrong_api) else {
        return false;
    };
    validate_firmware_profile(function, &parsed)
        == Err(IntelFirmwareProfileError::ApiVersion {
            found: 76,
            min: 77,
            max: 77,
        })
}

pub fn stage13_10af_ax201_mmio_startup_self_test() -> bool {
    let Some(function) = crate::wifi_hw::stage13_10i_ax201_supported_function() else {
        return false;
    };
    let Ok(page) = crate::wifi_hw::DmaPage::allocate_zeroed() else {
        return false;
    };
    let Ok(region) =
        crate::wifi_hw::MmioRegion::new(page.physical_address(), crate::wifi_hw::DMA_PAGE_SIZE)
    else {
        return false;
    };
    // SAFETY: this test owns the page exclusively and uses it as synthetic
    // MMIO for the same volatile adapter used by the physical path.
    let registers =
        unsafe { crate::wifi_hw::VolatileMmio32::from_mapped(region, page.virtual_address()) };
    let Ok(io) = crate::wifi_hw::Intel22000MmioIo::from_supported(function, registers) else {
        return false;
    };
    let mut context = Intel22000DmaContextInfo::new();
    if context.set_rx_queue(0x100000, 0x110000, 0x120000).is_err()
        || context
            .set_command_queue(0x130000, INTEL_CONTEXT_INFO_COMMAND_QUEUE_CB_SIZE)
            .is_err()
        || context
            .stage_firmware_chunk(IntelContextInfoImageKind::Lmac, &[0x31; 512])
            .is_err()
        || context
            .stage_firmware_chunk(IntelContextInfoImageKind::Umac, &[0x42; 256])
            .is_err()
    {
        return false;
    }
    let Ok(hardware_context) = context.build_hardware_context(0x2200) else {
        return false;
    };

    let mut startup = Intel22000FirmwareStartup::new(io);
    if startup.publish_context(&hardware_context).is_err()
        || startup.issue_cpu_init_run().is_err()
        || startup.begin_alive_wait().is_err()
        || startup.state() != IntelFirmwareStartState::AwaitingAlive
    {
        return false;
    }

    page.read_u32(INTEL_CSR_CONTEXT_INFO_BASE as usize)
        == Ok(hardware_context.physical_address() as u32)
        && page.read_u32(INTEL_CSR_CONTEXT_INFO_BASE as usize + 4)
            == Ok((hardware_context.physical_address() >> 32) as u32)
        && page.read_u32(crate::wifi_hw::INTEL_HBUS_PRPH_WADDR)
            == Ok(crate::wifi_hw::INTEL_PRPH_DWORD_ACCESS | INTEL_UREG_CPU_INIT_RUN)
        && page.read_u32(crate::wifi_hw::INTEL_HBUS_PRPH_WDAT) == Ok(INTEL_CPU_INIT_RUN_VALUE)
}

pub fn stage13_10ag_ax201_driver_lifecycle_self_test() -> bool {
    let Some(function) = crate::wifi_hw::stage13_10i_ax201_supported_function() else {
        return false;
    };
    let Ok(page) = crate::wifi_hw::DmaPage::allocate_zeroed() else {
        return false;
    };
    let Ok(region) =
        crate::wifi_hw::MmioRegion::new(page.physical_address(), crate::wifi_hw::DMA_PAGE_SIZE)
    else {
        return false;
    };
    // SAFETY: the test keeps the backing page alive until the driver is
    // stopped and the volatile adapter is dropped.
    let registers =
        unsafe { crate::wifi_hw::VolatileMmio32::from_mapped(region, page.virtual_address()) };
    let Ok(io) = crate::wifi_hw::Intel22000MmioIo::from_supported(function, registers) else {
        return false;
    };

    let mut bytes = [0u8; INTEL_TLV_UCODE_HEADER_SIZE + 5 * 16 + 12];
    bytes[4..8].copy_from_slice(&INTEL_TLV_UCODE_MAGIC.to_le_bytes());
    bytes[72..76].copy_from_slice(&0x0000_4d00u32.to_le_bytes());
    for (index, offset) in [
        0x1000u32,
        INTEL_CPU_SEPARATOR,
        0x2000,
        INTEL_PAGING_SEPARATOR,
        0x3000,
    ]
    .into_iter()
    .enumerate()
    {
        let start = INTEL_TLV_UCODE_HEADER_SIZE + index * 16;
        bytes[start..start + 4].copy_from_slice(&INTEL_TLV_SEC_RT.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&8u32.to_le_bytes());
        bytes[start + 8..start + 12].copy_from_slice(&offset.to_le_bytes());
        if matches!(offset, INTEL_CPU_SEPARATOR | INTEL_PAGING_SEPARATOR) {
            bytes[start + 12..start + 16].fill(0);
        } else {
            bytes[start + 12..start + 16].fill(index as u8 + 1);
        }
    }
    let paging = INTEL_TLV_UCODE_HEADER_SIZE + 5 * 16;
    bytes[paging..paging + 4].copy_from_slice(&INTEL_TLV_PAGING.to_le_bytes());
    bytes[paging + 4..paging + 8].copy_from_slice(&4u32.to_le_bytes());
    bytes[paging + 8..paging + 12].copy_from_slice(&4096u32.to_le_bytes());

    let wrong_source = Ax201FirmwareSource {
        filename: "iwlwifi-cc-a0-77.ucode",
        bytes: &bytes,
    };
    if validate_ax201_firmware_source(function, wrong_source)
        != Err(Intel22000FirmwarePrepareError::Profile(
            IntelFirmwareProfileError::Filename {
                expected: "iwlwifi-QuZ-a0-hr-b0-77.ucode",
            },
        ))
    {
        return false;
    }
    let source = Ax201FirmwareSource {
        filename: "iwlwifi-QuZ-a0-hr-b0-77.ucode",
        bytes: &bytes,
    };
    let Ok(mut driver) = Ax201Driver::new_from_source(function, source, io) else {
        return false;
    };
    if driver.allocate_and_attach_queues().is_err()
        || driver.start(0x2200).is_err()
        || driver.state() != Ax201DriverState::AwaitingAlive
    {
        return false;
    }
    let alive = stage13_10t_valid_payload(INTEL_ALIVE_STATUS_OK);
    if driver
        .handle_alive(INTEL_UCODE_ALIVE_NTFY, IntelAliveAbiVersion::V3, &alive)
        .is_err()
        || driver.state() != Ax201DriverState::Running
    {
        return false;
    }
    let scan_channels = [Ax201ScanChannel {
        number: 1,
        band: 0,
        flags: 0,
        iteration_count: 1,
        iteration_interval: 1,
    }];
    let Ok(scan_sequence) = driver.post_scan_request_v17(0x0102_0304, &scan_channels, 20) else {
        return false;
    };
    let command_response_payload = [0u8; 4];
    let mut command_response_packet = [0u8; INTEL_RX_PACKET_HEADER_SIZE + 4];
    let Some(command_response_packet) = stage13_10w_packet_into(
        &mut command_response_packet,
        INTEL_SCAN_REQUEST_UMAC_CMD,
        INTEL_SCAN_GROUP,
        scan_sequence,
        &command_response_payload,
    ) else {
        return false;
    };
    if driver
        .complete_successful_command_response(command_response_packet)
        .is_err()
        || driver.post_scan_request_v17(0x0506_0708, &scan_channels, 20)
            != Err(Ax201DriverError::Scan(Ax201ScanError::ScanAlreadyActive))
        || driver.post_scan_abort(0x0506_0708)
            != Err(Ax201DriverError::Scan(Ax201ScanError::ScanUidMismatch))
    {
        return false;
    }
    let mut complete_payload = [0u8; 16];
    complete_payload[0..4].copy_from_slice(&0x0102_0304u32.to_le_bytes());
    complete_payload[4..8].copy_from_slice(&[0, 0, 1, 0]);
    let mut complete_packet = [0u8; INTEL_RX_PACKET_HEADER_SIZE + 16];
    let Some(complete_packet) = stage13_10w_packet_into(
        &mut complete_packet,
        INTEL_SCAN_COMPLETE_UMAC_CMD,
        INTEL_SCAN_GROUP,
        0x8001,
        &complete_payload,
    ) else {
        return false;
    };
    if !matches!(
        driver.handle_scan_notification(complete_packet),
        Ok(Ax201ScanNotification::Complete(_))
    ) || driver.handle_scan_notification(complete_packet)
        != Err(Ax201DriverError::Scan(Ax201ScanError::ScanNotActive))
    {
        return false;
    }
    driver.mark_hardware_quiesced();
    driver.stop();
    driver.state() == Ax201DriverState::Stopped
}

pub fn stage13_10ah_ax201_firmware_provider_self_test() -> bool {
    let Some(function) = crate::wifi_hw::stage13_10i_ax201_supported_function() else {
        return false;
    };
    let Some(expected) = ax201_firmware_filename(function) else {
        return false;
    };
    validate_ax201_firmware_source(
        function,
        Ax201FirmwareSource {
            filename: expected,
            bytes: &[],
        },
    )
    .is_ok()
        && validate_ax201_firmware_source(
            function,
            Ax201FirmwareSource {
                filename: "iwlwifi-cc-a0-77.ucode",
                bytes: &[],
            },
        ) == Err(Intel22000FirmwarePrepareError::Profile(
            IntelFirmwareProfileError::Filename { expected },
        ))
}

pub fn stage13_10aj_ax201_scan_control_self_test() -> bool {
    let config = Ax201ScanConfig {
        enable_cam_mode: 1,
        enable_promiscuous_mode: 0,
        broadcast_station_id: 0xff,
        tx_chains: 0x0000_0003,
        rx_chains: 0x0000_0003,
    };
    let mut config_bytes = [0u8; Ax201ScanConfig::PAYLOAD_BYTES];
    if config.encode(&mut config_bytes).is_err()
        || config_bytes[0..4] != [1, 0, 0xff, 0]
        || config_bytes[4..8] != 0x0000_0003u32.to_le_bytes()
        || config_bytes[8..12] != 0x0000_0003u32.to_le_bytes()
        || (Ax201ScanConfig {
            tx_chains: 0,
            ..config
        })
        .encode(&mut config_bytes)
            != Err(Ax201ScanError::InvalidChains)
    {
        return false;
    }

    let mut abort = [0u8; 8];
    if encode_ax201_scan_abort(0x1234_5678, &mut abort).is_err()
        || abort != [0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0]
    {
        return false;
    }
    if encode_ax201_scan_abort(0, &mut abort) != Err(Ax201ScanError::InvalidUid) {
        return false;
    }

    let channels = [
        Ax201ScanChannel {
            number: 1,
            band: 0,
            flags: 0,
            iteration_count: 1,
            iteration_interval: 1,
        },
        Ax201ScanChannel {
            number: 36,
            band: 1,
            flags: 0,
            iteration_count: 2,
            iteration_interval: 3,
        },
    ];
    let mut request = [0u8; AX201_SCAN_REQUEST_V17_BYTES];
    if encode_ax201_scan_request_v17(0x0102_0304, &channels, 20, &mut request).is_err()
        || request[0..4] != 0x0102_0304u32.to_le_bytes()
        || request[8..10] != (1u16 << 11 | 1u16 << 2).to_le_bytes()
        || request[40] != 20
        || request[41] != 20
        || request[45] != 2
        || request[48 + 4] != 1
        || request[48 + 6] != 1
        || request[48 + 7] != 1
        || request[56 + 4] != 36
        || request[56 + 6] != 2
        || request[56 + 7] != 3
        || encode_ax201_scan_request_v17(1, &[], 20, &mut request)
            != Err(Ax201ScanError::InvalidChannels)
    {
        return false;
    }
    let ssid = b"WovenHat";
    if encode_ax201_active_scan_request_v17(
        0x0102_0304,
        &channels,
        20,
        [0x02, 0x57, 0x48, 0x13, 0x09, 0x04],
        &[ssid],
        &mut request,
    )
    .is_err()
        || request[8..10] != (1u16 << 2).to_le_bytes()
        || request[44 + 4..44 + 8] != (0x0001_0000u32).to_le_bytes()
        || request[596..598] != [0, 0]
        || request[596 + 16..596 + 18] != 24u16.to_le_bytes()
        || request[1132..1136] != [0, 8, b'W', b'o']
        || encode_ax201_active_scan_request_v17(1, &channels, 20, [0xff; 6], &[], &mut request)
            != Err(Ax201ScanError::InvalidProbeAddress)
    {
        return false;
    }

    let mut management = [0u8; crate::wifi80211::MGMT_HEADER_LEN + 4];
    if crate::wifi80211::build_management_header(
        &mut management,
        11,
        [0x02, 0x57, 0x48, 0x13, 0x09, 0x55],
        [0x02, 0x57, 0x48, 0x13, 0x09, 0x44],
        [0x02, 0x57, 0x48, 0x13, 0x09, 0x55],
        0,
    )
    .is_err()
    {
        return false;
    }
    management[24..].copy_from_slice(&[0, 0, 1, 0]);
    let mut tx = [0u8; AX201_TX_CMD_V9_HEADER_BYTES + 28];
    let Ok(tx_len) = encode_ax201_tx_management_v9(
        &management,
        Ax201TxManagementOptions {
            rate_n_flags: 0x0000_0100,
            high_priority: true,
        },
        &mut tx,
    ) else {
        return false;
    };
    if tx_len != AX201_TX_CMD_V9_HEADER_BYTES + management.len()
        || tx[0..2] != 28u16.to_le_bytes()
        || tx[2..4] != [0, 0]
        || tx[4..8] != 5u32.to_le_bytes()
        || tx[16..20] != 0x0000_0100u32.to_le_bytes()
        || tx[AX201_TX_CMD_V9_HEADER_BYTES..tx_len] != management
        || encode_ax201_tx_management_v9(
            &[0u8; 23],
            Ax201TxManagementOptions {
                rate_n_flags: 0,
                high_priority: false,
            },
            &mut tx,
        ) != Err(Ax201TxError::InvalidManagementFrame)
    {
        return false;
    }

    let mut dqa = [0u8; AX201_DQA_ENABLE_PAYLOAD_BYTES];
    if encode_ax201_dqa_enable(&mut dqa).is_err()
        || dqa != u32::from(crate::wifi_hw::AX201_COMMAND_QUEUE_ID).to_le_bytes()
    {
        return false;
    }
    let mut queue_config = [0u8; AX201_SCD_QUEUE_CONFIG_V3_BYTES];
    if encode_ax201_scd_queue_config_v3(1, 0, 0, 2, 0x1234_5000, 0x2345_6000, &mut queue_config)
        .is_err()
        || queue_config[0..4] != [0, 0, 0, 0]
        || queue_config[4..8] != [1, 0, 0, 0]
        || queue_config[16..20] != [2, 0, 0, 0]
        || queue_config[20..28] != 0x1234_5000u64.to_le_bytes()
        || queue_config[28..36] != 0x2345_6000u64.to_le_bytes()
        || encode_ax201_scd_queue_config_v3(0, 0, 0, 2, 1, 1, &mut queue_config)
            != Err(Ax201TxError::InvalidQueue)
        || encode_ax201_scd_queue_config_v3(1, 0, 0, 6, 1, 1, &mut queue_config)
            != Err(Ax201TxError::InvalidQueueSize)
    {
        return false;
    }
    let mut queue_response = [0u8; AX201_TX_QUEUE_RESPONSE_BYTES];
    queue_response[0..2].copy_from_slice(&5u16.to_le_bytes());
    queue_response[4..6].copy_from_slice(&3u16.to_le_bytes());
    if parse_ax201_tx_queue_config_response(&queue_response)
        != Ok(Ax201TxQueueConfigResponse {
            queue_number: 5,
            write_pointer: 3,
        })
    {
        return false;
    }
    let mut tx_response_payload = [0u8; AX201_TX_RESPONSE_BYTES];
    tx_response_payload[0] = 1;
    tx_response_payload[21..25].copy_from_slice(&0x0102_0304u32.to_le_bytes());
    tx_response_payload[27..29].copy_from_slice(&28u16.to_le_bytes());
    tx_response_payload[31..33].copy_from_slice(&11u16.to_le_bytes());
    tx_response_payload[33..35].copy_from_slice(&5u16.to_le_bytes());
    tx_response_payload[37..39].copy_from_slice(&1u16.to_le_bytes());
    tx_response_payload[39..41].copy_from_slice(&0x0503u16.to_le_bytes());
    if parse_ax201_tx_response(IntelRxNotification {
        command: INTEL_TX_CMD,
        group_id: INTEL_LEGACY_GROUP,
        sequence: 0x0503,
        payload: &tx_response_payload,
    }) != Ok(Ax201TxResponse {
        frame_count: 1,
        tfd_info: 0x0102_0304,
        byte_count: 28,
        frame_control: 11,
        queue_number: 5,
        status: 1,
        sequence: 0x0503,
    }) {
        return false;
    }

    let mut complete_payload = [0u8; 16];
    complete_payload[0..4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
    complete_payload[4..8].copy_from_slice(&[2, 7, 0, 1]);
    complete_payload[8..12].copy_from_slice(&99u32.to_le_bytes());
    let mut packet_storage = [0u8; INTEL_RX_PACKET_HEADER_SIZE + 16];
    let Some(packet) = stage13_10w_packet_into(
        &mut packet_storage,
        INTEL_SCAN_COMPLETE_UMAC_CMD,
        INTEL_SCAN_GROUP,
        0x2209,
        &complete_payload,
    ) else {
        return false;
    };
    let Ok(notification) = parse_intel_rx_notification(packet) else {
        return false;
    };
    let Ok(complete) = parse_ax201_scan_complete(notification) else {
        return false;
    };
    complete.uid == 0x1234_5678
        && complete.last_schedule == 2
        && complete.last_iteration == 7
        && complete.status == 0
        && complete.ebs_status == 1
        && complete.time_from_last_iteration == 99
        && {
            let mut iteration_payload = [0u8; 32];
            iteration_payload[0..4].copy_from_slice(&0x0102_0304u32.to_le_bytes());
            iteration_payload[4..8].copy_from_slice(&[2, 1, 0, 36]);
            iteration_payload[8..16].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
            let mut iteration_storage = [0u8; INTEL_RX_PACKET_HEADER_SIZE + 32];
            let Some(iteration_packet) = stage13_10w_packet_into(
                &mut iteration_storage,
                INTEL_SCAN_ITERATION_COMPLETE_UMAC_CMD,
                INTEL_SCAN_GROUP,
                0x220a,
                &iteration_payload,
            ) else {
                return false;
            };
            let Ok(iteration_notification) = parse_intel_rx_notification(iteration_packet) else {
                return false;
            };
            let Ok(iteration) = parse_ax201_scan_iteration_complete(iteration_notification) else {
                return false;
            };
            iteration.uid == 0x0102_0304
                && iteration.scanned_channels == 2
                && iteration.status == 1
                && iteration.bluetooth_status == 0
                && iteration.last_channel == 36
                && iteration.start_tsf == 0x1122_3344_5566_7788
        }
}

pub fn stage13_10ak_ax201_phy_context_self_test() -> bool {
    let context = Ax201PhyContextV1 {
        id_and_color: 0x0000_0201,
        action: 1,
        apply_time: 0,
        tx_param_color: 0,
        channel: 36,
        band: 0,
        width: 2,
        ctrl_pos: 0,
        tx_chain_info: 0x0000_0003,
        rx_chain_info: 0x0000_0003,
        acquisition_data: 0,
        dsp_cfg_flags: 0,
    };
    let mut bytes = [0u8; Ax201PhyContextV1::PAYLOAD_BYTES];
    if context.encode(&mut bytes).is_err()
        || bytes[0..4] != 0x0000_0201u32.to_le_bytes()
        || bytes[4..8] != 1u32.to_le_bytes()
        || bytes[16..20] != 36u32.to_le_bytes()
        || bytes[20..24] != [0, 2, 0, 0]
        || bytes[24..28] != 0x0000_0003u32.to_le_bytes()
        || (Ax201PhyContextV1 {
            channel: 0,
            ..context
        })
        .encode(&mut bytes)
            != Err(Ax201PhyContextError::Channel)
        || (Ax201PhyContextV1 { band: 3, ..context }).encode(&mut bytes)
            != Err(Ax201PhyContextError::Band)
        || (Ax201PhyContextV1 {
            width: 5,
            ..context
        })
        .encode(&mut bytes)
            != Err(Ax201PhyContextError::Width)
    {
        return false;
    }
    true
}

pub fn stage13_10al_ax201_mac_binding_self_test() -> bool {
    let binding = Ax201BindingContextV2 {
        id_and_color: 0x0000_0201,
        action: 1,
        macs: [0x0000_0001, 0, 0],
        phy: 0x0000_0200,
        lmac_id: 0,
    };
    let mut binding_bytes = [0u8; Ax201BindingContextV2::PAYLOAD_BYTES];
    if binding.encode(&mut binding_bytes).is_err()
        || binding_bytes[0..4] != 0x0000_0201u32.to_le_bytes()
        || binding_bytes[8..12] != 1u32.to_le_bytes()
        || binding_bytes[20..24] != 0x0000_0200u32.to_le_bytes()
        || binding_bytes[24..28] != [0, 0, 0, 0]
    {
        return false;
    }

    let mac = Ax201MacContextStaV1 {
        id_and_color: 1,
        action: 1,
        mac_type: Ax201MacContextStaV1::BSS_STA_MAC_TYPE,
        tsf_id: 0,
        node_addr: [0x02, 0x11, 0x22, 0x33, 0x44, 0x55],
        bssid_addr: [0; 6],
        cck_rates: 0,
        ofdm_rates: 0,
        protection_flags: 0,
        cck_short_preamble: 0x20,
        short_slot: 0x10,
        filter_flags: 1 << 6,
        qos_flags: 0,
        ac: [Ax201MacQos {
            cw_min: 3,
            cw_max: 7,
            aifsn: 2,
            fifos_mask: 0,
            edca_txop: 0,
        }; 5],
        is_assoc: 0,
        dtim_time: 0,
        dtim_tsf: 0,
        beacon_interval: 0,
        reserved: 0,
        dtim_interval: 0,
        data_policy: 0,
        listen_interval: 0,
        assoc_id: 0,
        assoc_beacon_arrive_time: 0,
        p2p_ctwin: 0,
    };
    let mut mac_bytes = [0u8; Ax201MacContextStaV1::PAYLOAD_BYTES];
    mac.encode(&mut mac_bytes).is_ok()
        && mac_bytes[16..22] == [0x02, 0x11, 0x22, 0x33, 0x44, 0x55]
        && mac_bytes[60..68] == [3, 0, 7, 0, 2, 0, 0, 0]
        && mac_bytes[100..104] == [0, 0, 0, 0]
        && (Ax201MacContextStaV1 {
            node_addr: [2, 2, 3, 4, 5, 6],
            tsf_id: 4,
            ..mac
        })
        .encode(&mut mac_bytes)
            == Err(Ax201MacContextError::InvalidTsf)
}
