"""Stage 19.2: hello-gui presents a window through the compositor.

With --stage 19.4 the same image runs the compositor authority gate: an
unprivileged --probe run that must be refused, then a run with DisplayIo.
With --stage 19.5 it runs the input focus gate with hello-gui --input, and
with --stage 19.6 the blocking input wait gate with hello-gui --wait. With
--stage 19.7 it runs hello-gui as a compositor process and as its client, and
with --stage 19.8 the combined input/IPC wait gate with hello-gui --multiplex.
"""
import argparse, os, shutil, subprocess, sys
from pathlib import Path
APP='wovenhat-hello-gui'
STAGES={
    '19.2': ('stage19-2-test', '[S19.2] hello-gui through the compositor: PASSED', 'hello-gui-compositor', 'hello-gui compositor'),
    '19.4': ('stage19-4-test', '[S19.4] compositor authority: PASSED', 'hello-gui-authority', 'compositor authority'),
    '19.5': ('stage19-5-test', '[S19.5] input focus: PASSED', 'hello-gui-input', 'input focus'),
    '19.6': ('stage19-6-test', '[S19.6] blocking input wait: PASSED', 'hello-gui-wait', 'blocking input wait'),
    '19.7': ('stage19-7-test', '[S19.7] surface delegation: PASSED', 'hello-gui-delegation', 'surface delegation'),
    '19.8': ('stage19-8-test', '[S19.8] combined wait: PASSED', 'hello-gui-multiplex', 'combined wait'),
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


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--qemu',default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    p.add_argument('--firmware',type=Path)
    p.add_argument('--firmware-vars',type=Path)
    p.add_argument('--cpus',type=int,choices=(1,2,4),default=1)
    p.add_argument('--stage',choices=tuple(STAGES),default='19.2')
    p.add_argument('--timeout',type=float,default=300)
    a=p.parse_args()
    feature,marker,out_name,label=STAGES[a.stage]
    root=Path(__file__).resolve().parents[1]
    qemu=Path(a.qemu); fw=a.firmware or qemu.parent/'share'/'edk2-x86_64-code.fd'
    if not qemu.is_file() or not fw.is_file(): p.error('qemu or firmware missing')
    app_dir=root/'apps'/'hello-gui'
    if subprocess.run(['cargo','build','--release'],cwd=app_dir,env=app_build_env(app_dir)).returncode: return 1
    image=app_dir/'target'/'x86_64-unknown-none'/'release'/APP
    env=dict(os.environ)
    env['WOVENHAT_HELLO_GUI_IMAGE']=str(image)
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
        if '[S19.' in line or 'hello-gui' in line: print('  '+line.strip())
    if code != 33 or marker not in text:
        print(f'FAILED (exit {code}). Evidence: {out}',file=sys.stderr)
        print(text[-3000:],file=sys.stderr); return 1
    print(f'{label}: PASS ({a.cpus} CPU, exit 33). Evidence: {out}')
    return 0
if __name__=='__main__': raise SystemExit(main())
