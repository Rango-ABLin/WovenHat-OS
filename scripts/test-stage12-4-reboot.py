"""Stage 12.4 persistent-disk rollback recovery acceptance test."""
import argparse, os, shutil, struct, subprocess, sys
from pathlib import Path
S=512; N=131072; R=32; F=1009
def format_disk(p):
    b=bytearray(S*N); v=memoryview(b)[:S]; v[0:3]=b"\xeb\x58\x90"; v[3:11]=b"WOVENHAT"; struct.pack_into("<H",v,11,S); v[13]=1; struct.pack_into("<H",v,14,R); v[16]=2; v[21]=0xf8; struct.pack_into("<I",v,32,N); struct.pack_into("<I",v,36,F); struct.pack_into("<I",v,44,2); struct.pack_into("<H",v,48,1); struct.pack_into("<H",v,50,6); v[64]=0x80; v[66]=0x29; v[71:82]=b"WOVENHAT   "; v[82:90]=b"FAT32   "; v[510:512]=b"\x55\xaa"
    x=memoryview(b)[S:2*S]; struct.pack_into("<I",x,0,0x41615252); struct.pack_into("<I",x,484,0x61417272); struct.pack_into("<I",x,488,0xffffffff); struct.pack_into("<I",x,492,3); struct.pack_into("<I",x,508,0xaa550000); b[6*S:7*S]=b[:S]; b[7*S:8*S]=b[S:2*S]
    for n in range(2): struct.pack_into("<III",b,(R+n*F)*S,0x0ffffff8,0x0fffffff,0x0fffffff)
    p.write_bytes(b)
def main():
    a=argparse.ArgumentParser(); a.add_argument("--qemu",default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe"); a.add_argument("--firmware",type=Path); a.add_argument("--firmware-vars",type=Path); a.add_argument("--cpus",type=int,choices=(1,2,4),default=2); a.add_argument("--timeout",type=float,default=180); a=a.parse_args()
    root=Path(__file__).resolve().parents[1]; q=Path(a.qemu); fw=a.firmware or q.parent/"share"/"edk2-x86_64-code.fd"
    if not q.is_file() or not fw.is_file(): raise SystemExit("QEMU/firmware not found")
    image=subprocess.check_output(["cargo","run","--quiet","--features","stage12-4-reboot-test","--","--print-image"],cwd=root,text=True).strip(); out=root/"target"/f"stage12-4-reboot-{a.cpus}-debug"; out.mkdir(parents=True,exist_ok=True); disk=out/"rollback-fat32.img"; format_disk(disk)
    def boot(n,marker):
        serial=out/f"boot{n}-serial.log"; serial.write_text(""); vc=None
        if a.firmware_vars: vc=out/f"boot{n}-OVMF_VARS.fd"; shutil.copyfile(a.firmware_vars,vc)
        cmd=[str(q),"-accel","tcg,tb-size=128","-machine","pc","-m","256M","-smp",str(a.cpus),"-display","none","-serial",f"file:{serial}","-no-reboot","-device","isa-debug-exit,iobase=0xf4,iosize=0x04","-drive",f"if=pflash,unit=0,format=raw,readonly=on,file={fw}",*([] if vc is None else ["-drive",f"if=pflash,unit=1,format=raw,file={vc}"]),"-drive",f"if=none,id=boot,format=raw,readonly=on,file={image}","-device","virtio-blk-pci,drive=boot,bootindex=1","-drive",f"if=ide,index=0,media=disk,format=raw,file={disk}"]
        try: r=subprocess.run(cmd,cwd=root,capture_output=True,text=True,timeout=a.timeout,creationflags=subprocess.CREATE_NO_WINDOW if os.name=="nt" else 0)
        except subprocess.TimeoutExpired: raise RuntimeError(f"boot {n} timed out")
        log=serial.read_text(errors="replace"); (out/f"boot{n}-qemu.log").write_text(r.stdout+r.stderr)
        if r.returncode!=33 or marker not in log: print(log[-12000:],file=sys.stderr); raise RuntimeError(f"boot {n} failed")
    boot(1,"[S12.4R] POWERLOSS AFTER REPLAY BEFORE WSR1 ADVANCE"); boot(2,"[S12.4R] reboot recovery + idempotent replay: PASSED"); print("Stage 12.4 persistent-disk two-boot rollback recovery: PASS"); return 0
if __name__=="__main__": raise SystemExit(main())
