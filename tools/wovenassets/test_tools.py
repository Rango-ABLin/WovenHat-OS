import json,sys,tempfile,unittest
from pathlib import Path
from PIL import Image
ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT))
from compare_frames import compare
from validate_tokens import validate
class GraphicsHostTests(unittest.TestCase):
    def test_design_tokens(self):
        self.assertEqual(set(validate(ROOT/"wovenui-tokens.json")["themes"]),{"dark","light"})
    def test_identical_images(self):
        with tempfile.TemporaryDirectory() as td:
            p=Path(td)/"a.png";Image.new("RGBA",(8,6),(50,20,120,255)).save(p)
            self.assertTrue(compare(p,p)["pass"])
    def test_pixel_difference(self):
        with tempfile.TemporaryDirectory() as td:
            a=Path(td)/"a.png";b=Path(td)/"b.png"
            Image.new("RGBA",(8,6),(50,20,120,255)).save(a)
            Image.new("RGBA",(8,6),(52,20,120,255)).save(b)
            self.assertFalse(compare(a,b)["pass"])
            self.assertTrue(compare(a,b,max_abs=2,mean_abs=1)["pass"])
    def test_mismatch_resolution(self):
        with tempfile.TemporaryDirectory() as td:
            a=Path(td)/"a.png";b=Path(td)/"b.png"
            Image.new("RGB",(8,6)).save(a);Image.new("RGB",(7,6)).save(b)
            self.assertEqual(compare(a,b)["reason"],"dimension_mismatch")
if __name__=="__main__":unittest.main()
