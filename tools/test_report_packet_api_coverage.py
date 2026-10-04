import copy
import unittest
from pathlib import Path, PurePosixPath, PureWindowsPath
from unittest.mock import patch
from report_packet_api_coverage import ROOT, inputs, records, render, source_key


class PacketApiCoverage(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalogs, cls.sources = inputs()

    def test_source_keys_are_platform_independent(self):
        for path_type, root in [(PurePosixPath, '/project'), (PureWindowsPath, 'C:/project')]:
            base=path_type(root)
            self.assertEqual(source_key(base/'src'/'packet'/'typed.rs',base),'src/packet/typed.rs')

    def test_source_reads_explicitly_use_utf8(self):
        original=Path.read_text
        encodings=[]
        def checked(path,*args,**kwargs):
            encodings.append(kwargs.get('encoding'))
            return original(path,*args,**kwargs)
        with patch.object(Path,'read_text',checked):
            catalogs,sources=inputs()
        self.assertEqual(len(catalogs),14)
        self.assertIn('src/packet/typed.rs',sources)
        self.assertTrue(encodings)
        self.assertEqual(set(encodings),{'utf-8'})

    def test_full_inventory_matches_checked_snapshot(self):
        rows=records(self.catalogs,self.sources)
        self.assertEqual(len(rows),267)
        self.assertEqual(sum(len(r['protocol_ids']) for r in rows),3233)
        self.assertEqual(render(rows),(ROOT/'docs/packet-api-coverage.tsv').read_text(encoding='utf-8'))
        self.assertEqual(len({(r['state'],r['direction'],r['name']) for r in rows}),267)

    def test_state_direction_and_legacy_exception_remain_separate(self):
        rows=records(self.catalogs,self.sources)
        selected=[r for r in rows if r['name']=='held_item_slot']
        self.assertEqual({r['direction'] for r in selected},{'Clientbound','Serverbound'})
        legacy=[r for r in rows if r['name']=='legacy_server_list_ping']
        self.assertEqual(len(legacy),1);self.assertIsNone(legacy[0]['api'])
        self.assertEqual(len(legacy[0]['protocol_ids']),14)
        contextual={r['name'] for r in rows if r['route']=='typed incoming with caller dimension context'}
        self.assertEqual(contextual,{'map_chunk','update_light','chunk_biomes'})

    def test_unknown_incoming_name_and_removed_dispatch_fail(self):
        catalogs=copy.copy(self.catalogs);catalogs[776]=catalogs[776].replace('"debug_event"','"unknown_debug_event"')
        with self.assertRaises(ValueError): records(catalogs,self.sources)
        sources=copy.copy(self.sources);sources['src/packet/typed.rs']=sources['src/packet/typed.rs'].replace('"debug_event"','"removed_debug_event"')
        with self.assertRaises(ValueError): records(self.catalogs,sources)

    def test_unknown_outgoing_name_and_wrong_state_fail(self):
        catalogs=copy.copy(self.catalogs);catalogs[776]=catalogs[776].replace('"spectator_action"','"unknown_spectator_action"')
        with self.assertRaises(ValueError): records(catalogs,self.sources)
        catalogs=copy.copy(self.catalogs);catalogs[776]=catalogs[776].replace('"accept_code_of_conduct"','"edit_book"')
        with self.assertRaises(ValueError): records(catalogs,self.sources)

    def test_catalog_shape_family_and_duplicate_guards(self):
        catalogs=copy.copy(self.catalogs);catalogs.pop(763)
        with self.assertRaises(ValueError): records(catalogs,self.sources)
        catalogs=copy.copy(self.catalogs);catalogs[776]=catalogs[776].replace('id: 254,','identifier: 254,')
        with self.assertRaises(ValueError): records(catalogs,self.sources)
        catalogs=copy.copy(self.catalogs);catalogs[776]=catalogs[776].replace('name: "legacy_server_list_ping"','name: "set_protocol"')
        with self.assertRaises(ValueError): records(catalogs,self.sources)

    def test_status_and_handshake_are_explicitly_classified(self):
        for name in ('set_protocol','ping_start','server_info'):
            catalogs=copy.copy(self.catalogs);catalogs[776]=catalogs[776].replace('"'+name+'"','"unknown_'+name+'"')
            with self.subTest(name=name), self.assertRaises(ValueError): records(catalogs,self.sources)


if __name__ == '__main__': unittest.main()
