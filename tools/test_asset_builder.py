"""Safety and physical-material invariants of the model-neutral recipe helper."""
import copy,json,subprocess,sys,tempfile,unittest
from pathlib import Path
from asset_builder import build,ROOT

class BuilderTests(unittest.TestCase):
    def setUp(self):self.recipe=json.loads((ROOT/'assets/recipes/supply_crate.json').read_text())
    def test_paint_changes_never_change_the_material_geometry(self):
        first,_=build(self.recipe)
        for style in self.recipe['styles'].values():style['paint']='#010203'
        second,_=build(self.recipe)
        native=lambda text:'\n'.join(l for l in text.splitlines()if not l.startswith('paint '))
        self.assertEqual(native(first),native(second))
    def test_out_of_bounds_and_overlaps_fail_before_writing(self):
        r=copy.deepcopy(self.recipe);r['parts'][0]['shapes'].append({'op':'rect','at':[39,31],'size':[2,2],'style':'wood_body'})
        with self.assertRaisesRegex(ValueError,'outside'):build(r)
        r=copy.deepcopy(self.recipe);r['parts'][1]['shapes'].append({'op':'pixel','at':[10,10],'style':'wood_body'})
        with self.assertRaisesRegex(ValueError,'overlaps'):build(r)
    def test_bad_material_duplicate_names_and_unsupported_letters_fail(self):
        r=copy.deepcopy(self.recipe);r['styles']['wood_body']['material']='made_up_material'
        with self.assertRaisesRegex(ValueError,'unknown material'):build(r)
        r=copy.deepcopy(self.recipe);r['parts'][1]['name']=r['parts'][0]['name']
        with self.assertRaisesRegex(ValueError,'Duplicate part'):build(r)
        r=copy.deepcopy(self.recipe);r['parts'][0]['shapes'].append({'op':'text','at':[0,0],'text':'?','style':'ink'})
        with self.assertRaisesRegex(ValueError,'Unsupported'):build(r)
    def test_swept_clearance_warns_when_a_fixed_part_enters_the_crank_circle(self):
        r=json.loads((ROOT/'assets/recipes/iron_heron.json').read_text());_,checks=build(r);self.assertEqual(checks,[])
        cabinet=next(p for p in r['parts']if p['name']=='control_cabinet')
        cabinet['shapes'].append({'op':'rect','at':[100,75],'size':[4,4],'style':'iron_outline'})
        _,checks=build(r);self.assertTrue(any('keepout' in s for s in checks))
    def test_omitting_saved_parts_preserves_the_existing_asset_byte_for_byte(self):
        with tempfile.TemporaryDirectory()as folder:
            out=Path(folder)/'saved.asset';out.write_text('part original_saved_part\n');before=out.read_bytes()
            result=subprocess.run([sys.executable,str(ROOT/'tools/asset_builder.py'),str(ROOT/'assets/recipes/supply_crate.json'),'--out',str(out)],capture_output=True,text=True)
            self.assertNotEqual(result.returncode,0);self.assertEqual(out.read_bytes(),before);self.assertIn('remove existing named parts',result.stderr)
    def test_the_complete_machine_recipe_reproduces_the_saved_native_asset(self):
        r=json.loads((ROOT/'assets/recipes/iron_heron.json').read_text());text,checks=build(r)
        self.assertEqual(checks,[]);self.assertEqual(text,(ROOT/'assets/workshop/iron_heron_pumpjack.asset').read_text())

if __name__=='__main__':unittest.main()
