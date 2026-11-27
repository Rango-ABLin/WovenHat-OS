import json,tempfile,unittest
from pathlib import Path
from pack_icons import build,validate_svg
OK=b'<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h24v24H0z" fill="#abcdef"/></svg>'
class TestPackIcons(unittest.TestCase):
    def test_pack(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);(p/"a.svg").write_bytes(OK)
            result=build(p,p/"out",(16,24))
            self.assertEqual(len(result["icons"]),2)
            for item in result["icons"]:self.assertEqual((p/"out"/item["file"]).stat().st_size,item["size"]**2*4)
            self.assertEqual(json.loads((p/"out"/"manifest.json").read_text())["schema_version"],1)
    def test_script(self):
        with self.assertRaises(ValueError):validate_svg(b'<svg xmlns="http://www.w3.org/2000/svg"><script/></svg>')
    def test_external(self):
        with self.assertRaises(ValueError):validate_svg(b'<svg xmlns="http://www.w3.org/2000/svg"><use href="https://example.org"/></svg>')
    def test_doctype(self):
        with self.assertRaises(ValueError):validate_svg(b'<!DOCTYPE svg><svg xmlns="http://www.w3.org/2000/svg"/>')
    def test_stylesheet_is_rejected(self):
        with self.assertRaises(ValueError):
            validate_svg(b'<svg xmlns="http://www.w3.org/2000/svg"><style>@import url(https://example.org/a.css)</style></svg>')
    def test_embedded_data_url_is_rejected(self):
        with self.assertRaises(ValueError):
            validate_svg(b'<svg xmlns="http://www.w3.org/2000/svg"><rect fill="data:image/png;base64,AA=="/></svg>')
    def test_event_handler_is_rejected(self):
        with self.assertRaises(ValueError):
            validate_svg(b'<svg xmlns="http://www.w3.org/2000/svg"><rect onclick="alert(1)"/></svg>')
    def test_empty(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises(ValueError):build(d,Path(d)/"out")
if __name__=="__main__":unittest.main()
