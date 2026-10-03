#!/usr/bin/env python3
"""No-server acceptance tests: reject corrupted counters, failures and source fixtures."""
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest import mock
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT))
import verify as gate

class AcceptanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a=gate.read_json(ROOT/'historical/results.json')
        cls.b=gate.read_json(ROOT/'historical/continuation-results.json')
        cls.rows=cls.a['results']+cls.b['results']
        cls.audits=gate.read_json(ROOT/'strict-cleanup-assessment.json')['results']
        cls.text=(ROOT/'historical/servers/26.2/registry_tags_probe.log').read_text()

    def test_archived_evidence_accepts(self):
        self.assertEqual(gate.verify_historical(ROOT),{'families':14,'chunks':1110,'historical_verified':True})

    def test_every_family_transcript_accepts(self):
        for row in self.rows:
            with self.subTest(version=row['version']):
                text=(ROOT/'historical/servers'/row['version']/'registry_tags_probe.log').read_text()
                self.assertEqual(gate.assess_client(text,row['version'])['counters'],row['counters'])

    def test_no_missing_duplicate_or_extra_families(self):
        for rows in [self.rows[:-1],self.rows+[self.rows[0]],self.rows[:-1]+[self.rows[0]]]:
            with self.assertRaises(gate.AcceptanceError):gate.check_all_rows(rows)

    def test_rejects_error_marker_even_with_valid_summary(self):
        for marker in ['Error: decode failed','DECODE_ERROR name=map_chunk','HARNESS_TIMEOUT=90','thread \'main\' panicked at x']:
            with self.subTest(marker=marker),self.assertRaises(gate.AcceptanceError):
                gate.assess_client(self.text+'\n'+marker+'\n','26.2')

    def test_rejects_missing_lifecycle_or_dimension(self):
        for prefix in ['login ','compression negotiated:','DERIVED_SECTIONS=','joined dimension ','CLEAN_LOCAL_SHUTDOWN']:
            text='\n'.join(l for l in self.text.splitlines() if not l.startswith(prefix))
            with self.subTest(prefix=prefix),self.assertRaises(gate.AcceptanceError):gate.assess_client(text,'26.2')

    def test_rejects_wrong_counts_and_roundtrips(self):
        replacements=[('validated: 81 chunks','validated: 80 chunks'),('chunks=81 chunk_wire_equal=81','chunks=80 chunk_wire_equal=81'),('tags=1 chunks=81','tags=0 chunks=81'),('registry=29 tags=1','registry=28 tags=1'),('members=8644','members=8643'),('modern_registry_entries=398','modern_registry_entries=397'),('sections=24 elapsed_ms','sections=16 elapsed_ms'),('elapsed_ms=20012','elapsed_ms=19012'),('chunk_wire_equal=81','chunk_wire_equal=80')]
        for a,b in replacements:
            # Replace only the last aggregate occurrence when there are several.
            at=self.text.rfind(a);self.assertGreaterEqual(at,0)
            text=self.text[:at]+self.text[at:].replace(a,b,1)
            with self.subTest(change=(a,b)),self.assertRaises(gate.AcceptanceError):gate.assess_client(text,'26.2')

    def test_rejects_missing_observed_keepalive_or_chunk(self):
        for prefix in ['keepalive acknowledged: ','chunk ']:
            lines=self.text.splitlines();i=next(i for i,l in enumerate(lines) if l.startswith(prefix));lines.pop(i)
            with self.subTest(prefix=prefix),self.assertRaises(gate.AcceptanceError):gate.assess_client('\n'.join(lines),'26.2')

    def test_canonical_wire_exception_is_narrow(self):
        for v in ['1.20.1','1.21.5']:
            text=(ROOT/'historical/servers'/v/'registry_tags_probe.log').read_text()
            self.assertEqual(gate.assess_client(text,v)['roundtrips']['chunk_wire_equal'],0)
        with self.assertRaises(gate.AcceptanceError):gate.assess_client(self.text.replace('chunk_wire_equal=81','chunk_wire_equal=0'),'26.2')

    def row_check(self,row,allow=False):
        audit=next(x['plugin_safety'] for x in self.audits if x['version']==row['version'])
        return gate.check_row(row,ROOT/'historical',self.a['binaries'],audit,allow)

    def test_failed_exit_or_source_binary_cannot_be_waived(self):
        for mutation in ['exit','binary','counter','server_exit','eula','listener','error']:
            row=copy.deepcopy(self.rows[-1])
            if mutation=='exit':row['tests'][1]['exit']=1
            elif mutation=='binary':row['tests'][1]['binary_sha256']='0'*64
            elif mutation=='counter':row['counters']['chunks']-=1
            elif mutation=='server_exit':row['server_exit']=1
            elif mutation=='eula':row['eula_copy_after']='0'*64
            elif mutation=='listener':row['listeners'][0]['address']='00000000:63DD'
            elif mutation=='error':row['error']='decoder failed'
            with self.subTest(mutation=mutation),self.assertRaises(gate.AcceptanceError):self.row_check(row,True)

    def test_historical_amendment_preserves_only_known_cleanup_failure(self):
        row=copy.deepcopy(self.rows[3]);self.row_check(row,True)
        with self.assertRaises(gate.AcceptanceError):self.row_check(row,False)
        row['tests'][1]['exit']=1
        with self.assertRaises(gate.AcceptanceError):self.row_check(row,True)
        row=copy.deepcopy(self.rows[-1]);row['passed']=False;row['cleanup_failure']=True
        with self.assertRaises(gate.AcceptanceError):self.row_check(row,True)

    def test_strict_plugin_classifier_evidence_cannot_be_broadened(self):
        valid=self.audits[3]['plugin_safety'];gate.check_plugin_audit(valid)
        for mutation in ['jar','mapping','index','missing_index','nonzero','unrecognized']:
            audit=copy.deepcopy(valid);cache=audit['generated_remap_classpath_jars'][0]
            if mutation=='jar':cache['path']='plugins/arbitrary.jar'
            elif mutation=='mapping':cache['mapping_path']='plugins/wrong.tiny'
            elif mutation=='index':cache['empty_indexes'][0]['contents']['hashes']={'plugin':'anything'}
            elif mutation=='missing_index':cache['empty_indexes'].pop()
            elif mutation=='nonzero':audit['plugin_initialization_evidence']=['Initialized 1 plugins']
            elif mutation=='unrecognized':audit['installed_or_unrecognized_plugin_jars']=['plugins/extra.jar']
            with self.subTest(mutation=mutation),self.assertRaises(gate.AcceptanceError):gate.check_plugin_audit(audit)

    def test_source_preflight_rejects_fixture_mutation(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);shutil.copytree(ROOT/'fixtures',root/'fixtures')
            p=root/'fixtures/registry_tags_probe.rs';p.write_text(p.read_text()+'\n// altered\n')
            with self.assertRaisesRegex(gate.AcceptanceError,'SHA-256 mismatch'):gate.verify_source(root)

    def test_source_preflight_rejects_archive_mismatch(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);shutil.copytree(ROOT/'fixtures',root/'fixtures');(root/'source.tar').write_bytes(b'wrong commit')
            with self.assertRaisesRegex(gate.AcceptanceError,'SHA-256 mismatch'):gate.verify_source(root)

    def test_source_preflight_rejects_tree_mismatch_even_when_archive_check_passes(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);(root/'source').mkdir();(root/'source/Cargo.toml').write_text('modified')
            (root/'fixtures').mkdir();shutil.copyfile(ROOT/'fixtures/source-sha256.json',root/'fixtures/source-sha256.json')
            with mock.patch.object(gate,'check_fixtures'),mock.patch.object(gate,'expect_sha'):
                with self.assertRaisesRegex(gate.AcceptanceError,'source tree differs'):gate.verify_source(root)

if __name__=='__main__':unittest.main()
