import hashlib,json,tempfile,unittest
from pathlib import Path
from PIL import Image
from qualify_guest_frame import qualify
class BaselineTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.ref=self.root/"approved.png";self.guest=self.root/"guest.png";self.manifest=self.root/"manifest.json"
        Image.new("RGB",(24,16),"#203040").save(self.ref)
        Image.new("RGB",(24,16),"#203040").save(self.guest)
        self.b={"id":"desktop","path":"approved.png","sha256":hashlib.sha256(self.ref.read_bytes()).hexdigest(),"size":[24,16],"reviewed_by":"reviewer","guest_commit":"deadbeef","capture_method":"QEMU screenshot","max_abs":0,"mean_abs":0.0}
        self.write()
    def write(self):
        self.manifest.write_text(json.dumps({"schema_version":1,"kind":"wovenhat-guest-baselines","baselines":[self.b]}))
    def test_match(self):self.assertTrue(qualify(self.manifest,"desktop",self.guest)["pass"])
    def test_pixel_change(self):
        Image.new("RGB",(24,16),"#203041").save(self.guest)
        self.assertFalse(qualify(self.manifest,"desktop",self.guest)["pass"])
    def test_modified_baseline(self):
        Image.new("RGB",(24,16),"red").save(self.ref)
        with self.assertRaisesRegex(ValueError,"hash mismatch"):qualify(self.manifest,"desktop",self.guest)
    def test_unreviewed(self):
        self.b["reviewed_by"]="";self.write()
        with self.assertRaisesRegex(ValueError,"reviewed_by"):qualify(self.manifest,"desktop",self.guest)
    def test_traversal(self):
        self.b["path"]="../approved.png";self.write()
        with self.assertRaisesRegex(ValueError,"unsafe"):qualify(self.manifest,"desktop",self.guest)
    def test_missing(self):
        self.ref.unlink()
        with self.assertRaisesRegex(ValueError,"missing"):qualify(self.manifest,"desktop",self.guest)
    def test_duplicate(self):
        self.manifest.write_text(json.dumps({"schema_version":1,"kind":"wovenhat-guest-baselines","baselines":[self.b,self.b]}))
        with self.assertRaisesRegex(ValueError,"uniquely"):qualify(self.manifest,"desktop",self.guest)
