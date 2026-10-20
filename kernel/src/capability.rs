#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Capability {
    Console = 0,
    TimerRead = 1,
    TaskInspect = 2,
    TaskControl = 3,
    DeviceIo = 4,
    InterruptControl = 5,
    MemoryInspect = 6,
    FileRead = 7,
    FileWrite = 8,
    Ipc = 9,
    ProcessCreate = 10,
    NetworkIo = 11,
    StorageIo = 12,
    DisplayIo = 13,
    InputIo = 14,
    NetworkAdmin = 15,
    Camera = 16,
    Microphone = 17,
    Gpu = 18,
    Usb = 19,
    Clipboard = 20,
    Location = 21,
    Ai = 22,
    Secrets = 23,
    /// Play sound through WovenAudio.
    AudioPlayback = 24,
    /// Turn the machine off or restart it: the desktop session only.
    PowerControl = 25,
    /// Change a local account's own PIN: the desktop session only. Never
    /// granted to applications; see `accounts::change_pin`.
    AccountManage = 26,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilitySet {
    bits: u64,
}

impl CapabilitySet {
    pub const fn only(capability: Capability) -> Self {
        Self::empty().with(capability)
    }

    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn kernel_bootstrap() -> Self {
        Self::empty()
            .with(Capability::Console)
            .with(Capability::TimerRead)
            .with(Capability::TaskInspect)
            .with(Capability::TaskControl)
            .with(Capability::DeviceIo)
            .with(Capability::InterruptControl)
            .with(Capability::MemoryInspect)
            .with(Capability::FileRead)
            .with(Capability::FileWrite)
            .with(Capability::Ipc)
            .with(Capability::TimerRead)
            .with(Capability::ProcessCreate)
            .with(Capability::NetworkIo)
            .with(Capability::StorageIo)
            .with(Capability::DisplayIo)
            .with(Capability::InputIo)
            .with(Capability::NetworkAdmin)
            .with(Capability::Camera)
            .with(Capability::Microphone)
            .with(Capability::Gpu)
            .with(Capability::Usb)
            .with(Capability::Clipboard)
            .with(Capability::Location)
            .with(Capability::Ai)
            .with(Capability::Secrets)
            .with(Capability::AudioPlayback)
            .with(Capability::PowerControl)
            .with(Capability::AccountManage)
    }

    pub const fn userspace() -> Self {
        Self::empty()
            .with(Capability::Console)
            .with(Capability::FileRead)
            .with(Capability::FileWrite)
            .with(Capability::Ipc)
            .with(Capability::ProcessCreate)
            .with(Capability::NetworkIo)
    }

    /// User applications start without these sensitive authorities, but their
    /// sandbox ceiling permits an authorized permission broker to grant them.
    pub const fn userspace_ceiling() -> Self {
        Self::userspace()
            .with(Capability::Camera)
            .with(Capability::Microphone)
            .with(Capability::Gpu)
            .with(Capability::Usb)
            .with(Capability::Clipboard)
            .with(Capability::Location)
            .with(Capability::Ai)
            .with(Capability::Secrets)
            .with(Capability::AudioPlayback)
    }

    pub const fn with(self, capability: Capability) -> Self {
        Self {
            bits: self.bits | capability.bit(),
        }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    #[cfg(any(
        feature = "stage21-7-test",
        feature = "stage21-8-test",
        feature = "stage21-9-test",
        feature = "stage21-10-test",
        feature = "stage21-11-test"
    ))]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    pub const fn contains(self, capability: Capability) -> bool {
        self.bits & capability.bit() != 0
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub const fn is_subset_of(self, other: Self) -> bool {
        self.bits & !other.bits == 0
    }

    pub const fn without(self, capability: Capability) -> Self {
        Self {
            bits: self.bits & !capability.bit(),
        }
    }
}

impl Capability {
    pub const COUNT: usize = 27;
    pub const ALL: [Capability; Self::COUNT] = [
        Capability::Console,
        Capability::TimerRead,
        Capability::TaskInspect,
        Capability::TaskControl,
        Capability::DeviceIo,
        Capability::InterruptControl,
        Capability::MemoryInspect,
        Capability::FileRead,
        Capability::FileWrite,
        Capability::Ipc,
        Capability::ProcessCreate,
        Capability::NetworkIo,
        Capability::StorageIo,
        Capability::DisplayIo,
        Capability::InputIo,
        Capability::NetworkAdmin,
        Capability::Camera,
        Capability::Microphone,
        Capability::Gpu,
        Capability::Usb,
        Capability::Clipboard,
        Capability::Location,
        Capability::Ai,
        Capability::Secrets,
        Capability::AudioPlayback,
        Capability::PowerControl,
        Capability::AccountManage,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    const fn bit(self) -> u64 {
        1 << self as u8
    }
}
