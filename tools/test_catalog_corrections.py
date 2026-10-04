"""Test the pure correction function without importing the generating script.

AST extraction deliberately loads only this function, so clean checkout tests
never fetch schemas or run the generator's filesystem side effects.
"""
import ast
from pathlib import Path
import unittest

source = ast.parse((Path(__file__).parent / 'generate_catalog.py').read_text())
node = next(n for n in source.body if isinstance(n, ast.FunctionDef) and n.name == 'corrected_mapping')
scope = {}
exec(compile(ast.Module(body=[node], type_ignores=[]), '<catalog correction>', 'exec'), scope)
corrected = scope['corrected_mapping']
EXPECTED = {f'0x{i:02x}': n for i, n in enumerate([
    'settings', 'cookie_response', 'custom_payload', 'finish_configuration',
    'keep_alive', 'pong', 'resource_pack_receive', 'select_known_packs',
    'custom_report_details', 'server_links'])}


class CatalogCorrection(unittest.TestCase):
    def test_phantom_outbound_metadata_removed_only_in_four_families(self):
        for p in range(767, 771):
            value = corrected(p, 'configuration', 'toServer', EXPECTED)
            self.assertEqual(value, {i: EXPECTED[f'0x{i:02x}'] for i in range(8)})
        for p in [763, 764, 765, 766, 771, 772, 773, 774, 775, 776]:
            self.assertEqual(len(corrected(p, 'configuration', 'toServer', EXPECTED)), 10)

    def test_direction_and_state_not_changed(self):
        for p in range(767, 771):
            for state, direction in [('configuration', 'toClient'), ('play', 'toClient'), ('play', 'toServer')]:
                self.assertEqual(len(corrected(p, state, direction, EXPECTED)), 10)

    def test_exact_mapping_guard_rejects_changes(self):
        for key in EXPECTED:
            changed = dict(EXPECTED); changed[key] = 'changed'
            with self.assertRaises(SystemExit): corrected(767, 'configuration', 'toServer', changed)
            changed = dict(EXPECTED); del changed[key]
            with self.assertRaises(SystemExit): corrected(770, 'configuration', 'toServer', changed)
        changed = dict(EXPECTED); changed['0x0a'] = 'future'
        with self.assertRaises(SystemExit): corrected(769, 'configuration', 'toServer', changed)

    def test_existing_776_spectator_fix_is_preserved_and_guarded(self):
        raw = {hex(i): n for i, n in {0: 'first', 0x3e:'arm_animation', 0x3f:'spectator_action',
               0x40:'test_instance_block_action',0x41:'block_place',0x42:'use_item',0x43:'custom_click_action'}.items()}
        fixed = corrected(776, 'play', 'toServer', raw)
        self.assertEqual(fixed[0x3e], 'spectator_action'); self.assertEqual(fixed[0x3f], 'arm_animation')
        self.assertEqual(fixed[0x40], 'spectate'); self.assertEqual(fixed[0x44], 'custom_click_action')
        self.assertEqual(fixed[0], 'first')
        raw['0x40'] = 'unknown'
        with self.assertRaises(SystemExit): corrected(776, 'play', 'toServer', raw)


if __name__ == '__main__': unittest.main()
