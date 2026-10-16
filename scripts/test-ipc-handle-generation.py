"""Actual-source IPC allocator regressions; run in a serialized diagnostic slot.

Extract the actual IPC handle allocator, not a rewritten model. Paging,
scheduling, object reference counts and WovenGuard authorization are NOT modeled.
The authorization double deliberately panics if a test tries to use it.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def section(text, start, stop):
    if text.count(start) != 1 or text.count(stop) != 1:
        raise ValueError(f'extraction boundary changed: {start!r} / {stop!r}')
    begin = text.index(start)
    end = text.index(stop, begin)
    return text[begin:end]


PRELUDE = r'''
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error { HandleTableFull, InvalidHandle, AccessDenied }
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LineageId(pub u64);
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ServiceClass { RoundTrip }
pub enum Capability { Ipc }
mod wovenguard {
    use super::{Capability, LineageId};
    pub fn lineage_authorizes(_: LineageId, _: Capability) -> bool {
        panic!("authorization is outside this allocator-only probe")
    }
}
'''

TESTS = r'''
#[cfg(test)]
mod tests {
    use super::*;
    const MAX_GENERATION: u32 = 0x00ff_ffff;
    fn object(id: u64) -> ObjectRef {
        ObjectRef { id: EndpointObjectId(id), kind: HandleObjectKind::Endpoint }
    }

    #[test]
    fn ordinary_reuse_changes_the_handle_and_preserves_capacity() {
        let mut table = HandleSpace { owner: 77, ..HandleSpace::EMPTY };
        let old = table.allocate(object(1), 3, None).unwrap();
        assert!(table.lookup(old).is_ok());
        assert!(table.remove(old).is_ok());
        assert!(matches!(table.lookup_raw(old), Err(Error::InvalidHandle)));
        let fresh = table.allocate(object(2), 5, None).unwrap();
        assert!(fresh != old);
        assert!(fresh.slot() == old.slot());
        let entry = table.lookup(fresh).unwrap();
        assert_eq!(entry.object.id.as_u64(), 2);
        assert_eq!(entry.rights, 5);
        assert_eq!(table.owner, 77);
        for id in 1..MAX_HANDLES {
            table.allocate(object(10 + id as u64), 1, None).unwrap();
        }
        assert_eq!(table.count(), MAX_HANDLES);
        assert!(matches!(table.allocate(object(99), 1, None), Err(Error::HandleTableFull)));
    }

    #[test]
    fn tagged_metadata_round_trips_without_authorization_double() {
        let mut table = HandleSpace::EMPTY;
        let obj = ObjectRef { id: EndpointObjectId(8), kind: HandleObjectKind::SharedMemory };
        let handle = table.allocate_tagged(obj, 0x40, Some(LineageId(7)), Some(ServiceClass::RoundTrip)).unwrap();
        let entry = table.lookup_raw(handle).unwrap();
        assert!(entry.object == obj);
        assert_eq!(entry.rights, 0x40);
        assert!(entry.lineage == Some(LineageId(7)));
        assert!(entry.service_class == Some(ServiceClass::RoundTrip));
        assert!(table.remove(handle).is_ok());
        assert_eq!(table.count(), 0);
    }

    #[test]
    fn rollover_cannot_reissue_a_stale_same_process_handle() {
        let mut table = HandleSpace { owner: 77, ..HandleSpace::EMPTY };
        let stale = table.allocate(object(1), 1, None).unwrap();
        table.remove(stale).unwrap();
        // Shortcut only the elapsed generations, not the allocator logic.
        table.generations[0] = MAX_GENERATION - 1;
        let last = table.allocate(object(2), 1, None).unwrap();
        assert_eq!(last.generation(), MAX_GENERATION);
        table.remove(last).unwrap();
        let replacement = table.allocate(object(3), 1, None).unwrap();
        assert!(replacement != stale, "stale raw handle reissued: {}", stale.as_u32());
        assert!(matches!(table.lookup_raw(stale), Err(Error::InvalidHandle)));
    }

    #[test]
    fn exhausted_empty_slot_is_skipped_for_an_available_slot() {
        let mut table = HandleSpace::EMPTY;
        table.generations[0] = MAX_GENERATION;
        let handle = table.allocate(object(1), 1, None).unwrap();
        assert!(handle.slot() == Some(1), "allocator reused exhausted slot");
        assert_eq!(handle.generation(), 1);
    }

    #[test]
    fn all_exhausted_slots_refuse_allocation_permanently() {
        let mut table = HandleSpace::EMPTY;
        table.generations.fill(MAX_GENERATION - 1);
        let issued: Vec<_> = (0..MAX_HANDLES)
            .map(|index| table.allocate(object(index as u64 + 1), 1, None).unwrap())
            .collect();
        for handle in issued {
            assert_eq!(handle.generation(), MAX_GENERATION);
            table.remove(handle).unwrap();
        }
        assert_eq!(table.count(), 0);
        for _ in 0..2 {
            assert!(matches!(table.allocate(object(99), 1, None), Err(Error::HandleTableFull)));
        }
    }
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--expected-source-sha256', required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    source = root / 'kernel/src/ipc.rs'
    raw = source.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != args.expected_source_sha256.lower():
        raise ValueError(f'source changed: expected {args.expected_source_sha256}, got {digest}')
    text = raw.decode('utf-8').replace('\r\n', '\n')
    config = (root / 'kernel/src/config.rs').read_text(encoding='utf-8')
    declaration = 'pub const MAX_IPC_HANDLES_PER_PROCESS: usize = 16;'
    if config.count(declaration) != 1:
        raise ValueError('configured handle capacity changed; review the probe')
    extracted = '\n'.join([
        section(text, '#[derive(Clone, Copy, PartialEq, Eq)]\npub struct Handle(u32);',
                '/// Rights attached to a process-local handle.'),
        section(text, '#[derive(Clone, Copy, PartialEq, Eq)]\npub enum HandleObjectKind {',
                '#[derive(Clone, Copy, PartialEq, Eq)]\npub struct HandleInfo {'),
        section(text, '#[derive(Clone, Copy, PartialEq, Eq)]\nstruct ObjectRef {',
                '#[derive(Clone, Copy, PartialEq, Eq)]\nstruct SharedMapping {'),
        section(text, '#[derive(Clone, Copy)]\nstruct HandleEntry {',
                '#[derive(Clone, Copy, PartialEq, Eq)]\nstruct ServiceName {'),
    ])
    output = root / 'audit-artifacts' / f'ipc-generation-probe-{time.time_ns()}'
    output.mkdir(parents=True)
    rust = output / 'ipc-allocator.rs'
    rust.write_text('const MAX_HANDLES: usize = 16;\n' + PRELUDE + extracted + TESTS, encoding='utf-8')
    binary = output / 'ipc-allocator-tests.exe'
    commands = [
        ['rustc', '--version', '--verbose'],
        ['rustc', '--crate-name', 'ipc_allocator_probe', '--edition=2021', '--test',
         '-D', 'warnings', str(rust), '-o', str(binary)],
        [str(binary), '--nocapture'],
    ]
    records = []
    code = 0
    for index, command in enumerate(commands):
        result = subprocess.run(command, cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        log = output / f'command-{index}.log'
        log.write_text(result.stdout, encoding='utf-8')
        records.append({'argv': command, 'exit': result.returncode, 'log': log.name})
        print(result.stdout, end='')
        if result.returncode:
            code = result.returncode
            break
    manifest = {'source': str(source), 'source_sha256': digest,
                'config_sha256': hashlib.sha256((root / 'kernel/src/config.rs').read_bytes()).hexdigest(),
                'builder_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                'extracted_sha256': hashlib.sha256(extracted.encode('utf-8')).hexdigest(),
                'probe_sha256': hashlib.sha256(rust.read_bytes()).hexdigest(), 'commands': records}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    print(f'Evidence: {output}')
    return code


if __name__ == '__main__':
    raise SystemExit(main())
