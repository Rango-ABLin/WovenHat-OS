"""Stage 20.1 and 20.2: the WovenShell desktop.

Builds apps/woven-shell, embeds it in a stage20-1-test kernel and boots it in
QEMU. The kernel starts the shell, checks every part of the desktop against
its own independent computation, then kills the shell and checks the display
returns to the text terminal. With --stage 20.2 hello-gui is embedded too and
runs as desktop applications: windows, taskbar entries, the tray and
notifications, driven by real pointer and keyboard input.
"""
import argparse, os, re, shutil, subprocess, sys
from pathlib import Path
APP='wovenhat-woven-shell'
STAGES={
    '20.1': ('stage20-1-test', '[S20.1] woven-shell desktop: PASSED', 'woven-shell', 'woven-shell desktop'),
    '20.2': ('stage20-2-test', '[S20.2] desktop applications: PASSED', 'woven-shell-apps', 'desktop applications'),
    '20.3': ('stage20-3-test', '[S20.3] application launcher: PASSED', 'woven-shell-launcher', 'application launcher'),
    '20.4': ('stage20-4-test', '[S20.4] windows that move and close: PASSED', 'woven-shell-windows', 'windows that move and close'),
    '20.5': ('stage20-5-test', '[S20.5] window depth: PASSED', 'woven-shell-depth', 'window depth'),
    '20.6': ('stage20-6-test', '[S20.6] translucent overlays: PASSED', 'woven-shell-translucency', 'translucent overlays'),
    '20.7': ('stage20-7-test', '[S20.7] spring animation: PASSED', 'woven-shell-animation', 'spring animation'),
    '20.8': ('stage20-8-test', '[S20.8] tiling: PASSED', 'woven-shell-tiling', 'tiling'),
    '20.9': ('stage20-9-test', '[S20.9] notification center: PASSED', 'woven-shell-notifications', 'notification center'),
    '20.10': ('stage20-10-test', '[S20.10] quick settings: PASSED', 'woven-shell-quick-settings', 'quick settings'),
    '20.11': ('stage20-11-test', '[S20.11] lock screen: PASSED', 'woven-shell-lock', 'lock screen'),
    '20.12': ('stage20-12-test', '[S20.12] sign-in: PASSED', 'woven-shell-sign-in', 'sign-in'),
    '20.13': ('stage20-13-test', '[S20.13] workspaces: PASSED', 'woven-shell-workspaces', 'workspaces'),
}

def app_build_env(app_dir):
    """Build environment for an application crate.

    The kernel's Cargo directory overrides are dropped and the app's own
    target directory pinned, so the binary lands where this harness reads it.
    Inheriting CARGO_TARGET_DIR would build into the kernel's tree and leave a
    stale image to be embedded.
    """
    env = dict(os.environ)
    env.pop('CARGO_BUILD_BUILD_DIR', None)
    env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
    return env


def vfs_node_capacity(config_path: Path) -> int:
    """Read the real VFS per-file limit rather than maintaining a stale copy."""
    source = config_path.read_text(encoding='utf-8')
    match = re.search(
        r'^\\s*pub const VFS_NODE_CAPACITY:\\s*usize\\s*=\\s*(\\d+)\\s*(?:\\*\\s*(\\d+))?\\s*;',
        source, re.MULTILINE,
    )
    if match is None:
        raise ValueError(f'cannot parse VFS_NODE_CAPACITY in {config_path}')
    capacity = int(match.group(1)) * int(match.group(2) or 1)
    if capacity <= 0:
        raise ValueError(f'invalid VFS_NODE_CAPACITY in {config_path}')
    return capacity


def shell_image_size_error(size: int, capacity: int) -> str | None:
    """A VFS insert would fail if the shell ELF exceeds its node capacity."""
    if size <= 0:
        return f'WovenShell ELF is empty ({size} bytes)'
    if size > capacity:
        return (
            f'WovenShell ELF is {size} bytes, exceeding VFS_NODE_CAPACITY '
            f'({capacity} bytes); the guest would fail to install /bin/woven-shell. '
            'Reduce the image or design a reviewed file-storage change; '
            'do not weaken the acceptance gate.'
        )
    return None


def validate_shell_image(image: Path, config_path: Path) -> bool:
    try:
        capacity = vfs_node_capacity(config_path)
        size = image.stat().st_size
    except (OSError, ValueError) as error:
        print(f'FAILED: WovenShell image preflight: {error}', file=sys.stderr)
        return False
    error = shell_image_size_error(size, capacity)
    if error:
        print(f'FAILED: {error}', file=sys.stderr)
        return False
    return True


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--qemu',default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    p.add_argument('--firmware',type=Path)
    p.add_argument('--firmware-vars',type=Path)
    p.add_argument('--cpus',type=int,choices=(1,2,4),default=1)
    p.add_argument('--stage',choices=tuple(STAGES),default='20.1')
    p.add_argument('--timeout',type=float,default=300)
    a=p.parse_args()
    feature,marker,out_name,label=STAGES[a.stage]
    root=Path(__file__).resolve().parents[1]
    qemu=Path(a.qemu); fw=a.firmware or qemu.parent/'share'/'edk2-x86_64-code.fd'
    if not qemu.is_file() or not fw.is_file(): p.error('qemu or firmware missing')
    app_dir=root/'apps'/'woven-shell'
    if subprocess.run(['cargo','build','--release'],cwd=app_dir,env=app_build_env(app_dir)).returncode: return 1
    image=app_dir/'target'/'x86_64-unknown-none'/'release'/APP
    if not validate_shell_image(image, root/'kernel'/'src'/'config.rs'):
        return 1
    env=dict(os.environ)
    env['WOVENHAT_SHELL_IMAGE']=str(image)
    if a.stage in ('20.2','20.3','20.4','20.5','20.6','20.7','20.8','20.9','20.10','20.11','20.12','20.13'):
        gui_dir=root/'apps'/'hello-gui'
        if subprocess.run(['cargo','build','--release'],cwd=gui_dir,env=app_build_env(gui_dir)).returncode: return 1
        env['WOVENHAT_HELLO_GUI_IMAGE']=str(gui_dir/'target'/'x86_64-unknown-none'/'release'/'wovenhat-hello-gui')
    env.setdefault('CARGO_BUILD_BUILD_DIR',str(root/'.cargo-build'))
    env.setdefault('CARGO_TARGET_DIR',str(root/'target'))
    boot=subprocess.run(['cargo','run','--quiet','--features',feature,'--','--print-image'],
                        cwd=root,env=env,capture_output=True,text=True)
    if boot.returncode:
        print(boot.stdout,boot.stderr,file=sys.stderr); return 1
    boot_image=boot.stdout.strip()
    out=root/'target'/f'{out_name}-{a.cpus}cpu'; out.mkdir(parents=True,exist_ok=True)
    serial=out/'serial.log'; serial.write_text('')
    vars_copy=None
    if a.firmware_vars is not None:
        vars_copy=out/'OVMF_VARS.fd'; shutil.copyfile(a.firmware_vars,vars_copy)
    cmd=[str(qemu),'-accel','tcg,tb-size=128','-machine','q35','-m','256M','-smp',str(a.cpus),
         '-display','none','-serial',f'file:{serial}','-no-reboot',
         '-device','isa-debug-exit,iobase=0xf4,iosize=0x04',
         '-drive',f'if=pflash,format=raw,readonly=on,file={fw}',
         *(['-drive',f'if=pflash,unit=1,format=raw,file={vars_copy}'] if vars_copy else []),
         '-drive',f'if=none,id=boot,format=raw,readonly=on,file={boot_image}',
         '-device','virtio-blk-pci,drive=boot,bootindex=1']
    flags=subprocess.CREATE_NO_WINDOW if os.name=='nt' else 0
    with (out/'qemu.log').open('w') as log:
        try: code=subprocess.run(cmd,cwd=root,stdout=log,stderr=log,timeout=a.timeout,creationflags=flags).returncode
        except subprocess.TimeoutExpired:
            print(f'QEMU timeout. Evidence: {out}',file=sys.stderr); return 1
    text=serial.read_text(errors='replace') if serial.exists() else ''
    for line in text.splitlines():
        if '[S20.' in line or 'woven-shell' in line or 'hello-gui' in line: print('  '+line.strip())
    if code != 33 or marker not in text:
        print(f'FAILED (exit {code}). Evidence: {out}',file=sys.stderr)
        print(text[-3000:],file=sys.stderr); return 1
    print(f'{label}: PASS ({a.cpus} CPU, exit 33). Evidence: {out}')
    return 0
if __name__=='__main__': raise SystemExit(main())
