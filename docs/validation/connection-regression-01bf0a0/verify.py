#!/usr/bin/env python3
"""Offline acceptance checks for archived evidence and a freshly reproduced run."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re

COMMIT = '01bf0a0739199949cd4aa584738aaba0e49fadc3'
ARCHIVE_SHA = '8b7c319b6ce7cc668640244dfb79a4eafe29faa655f6336d76acea85ecce871a'
SOURCE_MANIFEST_SHA = '0df0bf4ad1dd7dfc263e3cb8721619ec3e5339f19da6b2d9ea8562c7c32c36b8'
PROBE_SHA = 'ff4e61201c58d706d9f109bbf263c0798e03147973885b5b8dec3c6202841ca9'
CARGO_TOML_SHA = 'a4e6d8075b273411a6677cc69cfe3b5746763a962e60ee1da864ac5b9984beb2'
CARGO_LOCK_SHA = 'aa88dcfcd2be4b23ca57c314e43e7ab6ccc4e9aadf61ac9f0c79ec5a46ebc8c1'
CLASSIFIER_SHA = '00aa559925daafaa568f98d660f8cb6e5d37578825c8cac55f78843733dd6ddf'
COMPACT_SHA = '183970d13b87c141e5ef981e33e07d4b3c3c366910c2e69927162ed7a636a280'
ASSESSMENT_SHA = '49f86334eae6324083c24bb1f60b975bf025e029826b1602e354a37effb6d7a5'
VERSIONS = ['1.20.1','1.20.2','1.20.4','1.20.6','1.21.1','1.21.3','1.21.4','1.21.5','1.21.6','1.21.8','1.21.10','1.21.11','26.1.2','26.2']
PROTOCOLS = dict(zip(VERSIONS,range(763,777)))
COUNT_KEYS = ['chunks','keepalives','teleports','registry_packets','configurations','registries']
HEX = re.compile(r'[0-9a-f]{64}')

class AcceptanceError(ValueError):
    pass

def need(condition, message):
    if not condition:
        raise AcceptanceError(message)

def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for data in iter(lambda:f.read(1024*1024),b''):
            h.update(data)
    return h.hexdigest()

def read_json(path):
    return json.loads(Path(path).read_text())

def expect_sha(path, expected):
    need(sha(path)==expected, f'SHA-256 mismatch: {path}')

def check_fixtures(root):
    for name,digest in [('registry_tags_probe.rs',PROBE_SHA),('Cargo.toml.fixture',CARGO_TOML_SHA),('Cargo.lock.fixture',CARGO_LOCK_SHA),('source-sha256.json',SOURCE_MANIFEST_SHA)]:
        expect_sha(root/'fixtures'/name,digest)

def verify_source(root):
    root=Path(root)
    check_fixtures(root)
    expect_sha(root/'source.tar',ARCHIVE_SHA)
    expected=read_json(root/'fixtures/source-sha256.json')
    actual={p.relative_to(root/'source').as_posix():sha(p) for p in (root/'source').rglob('*') if p.is_file()}
    need(actual==expected,'Frozen source tree differs from the pinned 177-file commit snapshot')
    for name,digest in [('registry_tags_probe.rs',PROBE_SHA),('Cargo.toml',CARGO_TOML_SHA),('Cargo.lock',CARGO_LOCK_SHA)]:
        expect_sha(root/'probe'/name,digest)
    expect_sha(root/'source/tools/paper/validate_chunk_updates.py',CLASSIFIER_SHA)
    return True

def one(lines,prefix):
    found=[line for line in lines if line.startswith(prefix)]
    need(len(found)==1,f'Expected exactly one {prefix!r}, got {len(found)}')
    return found[0]

def assess_client(text,version):
    need(version in PROTOCOLS,'Unexpected representative version')
    p=PROTOCOLS[version];lines=text.splitlines()
    need(not any(line.startswith(('Error:','DECODE_ERROR','UNSUPPORTED','RAW_SCENARIO','HARNESS_TIMEOUT','thread \'','thread "')) for line in lines),'Client error marker')
    need(lines.count('login RustwireTags')==1,'Offline login missing/duplicated')
    need(any(re.fullmatch(r'compression negotiated: Some\(\d+\)',l) for l in lines),'Compression not negotiated')
    need(lines.count('CLEAN_LOCAL_SHUTDOWN')==1,'Clean client shutdown missing/duplicated')
    summary=one(lines,'validated: ')
    m=re.fullmatch(r'validated: (\d+) chunks, (\d+) keepalives, (\d+) teleports, (\d+) registry packets, (\d+) configurations, (\d+) registries, ([\d.]+)s',summary)
    need(m is not None,'Malformed counters or duration')
    counters=dict(zip(COUNT_KEYS,map(int,m.groups()[:6])))
    elapsed=float(m.group(7));need(math.isfinite(elapsed) and 20<=elapsed<=90,'Login duration outside 20–90 seconds')
    semantic={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)',one(lines,'PROBE_SUMMARY '))}
    rt={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)',one(lines,'ROUNDTRIPS '))}
    need(set(semantic)=={'tags_packets','tag_registries','tags','members','modern_registry_entries','sections','elapsed_ms'},'Unexpected or missing semantic counters')
    need(set(rt)=={'registry','tags','chunks','chunk_wire_equal'},'Unexpected or missing roundtrip counters')
    need(20000<=semantic['elapsed_ms']<=90000 and abs(elapsed*1000-semantic['elapsed_ms'])<1.1,'Duration counters disagree')
    dimension=one(lines,'joined dimension ')
    dm=re.fullmatch(r'joined dimension (\S+): min_y=(-?\d+), height=(\d+), sections=(\d+)',dimension)
    need(dm is not None,'Missing dimension record')
    min_y,height,sections=map(int,dm.groups()[1:])
    need(height>0 and height%16==0 and min_y%16==0 and sections==height//16,'Invalid derived dimension geometry')
    need(one(lines,'DERIVED_SECTIONS=')==f'DERIVED_SECTIONS={sections}' and semantic['sections']==sections,'Dimension section counts disagree')
    chunks=[re.fullmatch(r'chunk -?\d+,-?\d+: (\d+) sections; first block ID Some\(\d+\)',l) for l in lines if l.startswith('chunk ')]
    need(len(chunks)>=4 and all(x and int(x.group(1))==sections for x in chunks),'Missing/malformed full chunks')
    observed={'chunks':len(chunks),'keepalives':sum(l.startswith('keepalive acknowledged: ') for l in lines),'teleports':sum(l.startswith('teleport acknowledged: ') for l in lines),'registry_packets':sum(l.startswith('registry wire: ') for l in lines),'configurations':sum(l.startswith('configuration complete: ') for l in lines)}
    need(all(counters[k]==v for k,v in observed.items()),'Summary disagrees with observed control/chunk packet lines')
    need(counters['keepalives']>=1 and counters['teleports']>=1 and counters['registries']>=1,'Required control/registry evidence missing')
    need(counters['configurations']==(0 if p==763 else 1),'Configuration epoch count incorrect')
    need((counters['registry_packets']==0) if p==763 else (counters['registry_packets']>=1),'Registry packets missing/unexpected')
    tag_lines=[l for l in lines if l.startswith('tags decoded: ')]
    need(len(tag_lines)>=1 and len(tag_lines)==semantic['tags_packets'],'Tag packet count disagrees')
    parsed=[re.fullmatch(r'tags decoded: state=(Play|Configuration), registries=(\d+), tags=(\d+), members=(\d+), bytes=(\d+)',l) for l in tag_lines]
    need(all(parsed),'Malformed tag observations')
    need(all(x.group(1)==('Play' if p==763 else 'Configuration') and int(x.group(5))>0 for x in parsed),'Unexpected tag state/empty packet')
    need(sum(int(x.group(2)) for x in parsed)==semantic['tag_registries'] and int(parsed[-1].group(3))==semantic['tags'] and int(parsed[-1].group(4))==semantic['members'],'Tag counters disagree')
    need(semantic['tags']>0 and semantic['members']>0,'No nonempty tags exercised')
    modern=[re.fullmatch(r'registry wire: (\S+), entries=(\d+), present_nbt=(\d+)',l) for l in lines if l.startswith('registry wire: ') and l!='registry wire: legacy compound']
    need(all(modern),'Malformed modern registry observations')
    need(sum(int(x.group(2)) for x in modern)==semantic['modern_registry_entries'],'Modern registry entry count disagrees')
    need(all(int(x.group(2))==int(x.group(3)) for x in modern),'Unexpected omitted known-pack data')
    if p>=766:
        need(len(modern)==counters['registry_packets'] and len({x.group(1) for x in modern})==counters['registries'],'Modern registry inventory mismatch')
    need(rt['registry']==counters['registry_packets'] and rt['tags']==semantic['tags_packets'] and rt['chunks']==counters['chunks'],'Incomplete semantic/wire roundtrip coverage')
    need(0<=rt['chunk_wire_equal']<=counters['chunks'],'Impossible chunk wire-equality count')
    if p not in [763,770]:
        need(rt['chunk_wire_equal']==counters['chunks'],'Unexpected canonical chunk wire difference')
    return {'counters':counters,'semantic':semantic,'roundtrips':rt,'dimension':dimension}

def check_plugin_audit(audit):
    need(audit['gate_version']=='paper-generated-remap-classpath-v2' and audit['passed'] is True,'Strict plugin gate failed')
    need(not audit['installed_or_unrecognized_plugin_jars'],'Installed/unrecognized plugin jar')
    initialized=audit['plugin_initialization_evidence']
    need(not any(re.search(r'Initialized [1-9]\d* plugins',l) for l in initialized),'Nonzero plugins initialized')
    zero=any(re.search(r'Initialized 0 plugins',l) for l in initialized)
    for item in audit['generated_remap_classpath_jars']:
        match=re.fullmatch(r'plugins/\.paper-remapped/remap-classpath/([0-9A-F]{64})\.jar',item['path'])
        need(match is not None,'Unrecognized generated cache route')
        stem=match.group(1)
        need(item['mapping_path']==f'plugins/.paper-remapped/mappings/reversed/{stem}.tiny','Wrong reversed mapping path')
        need(HEX.fullmatch(item['sha256']) and HEX.fullmatch(item['mapping_sha256']),'Missing generated-cache hashes')
        indexes=item['empty_indexes']
        expected_paths={f'plugins/.paper-remapped/{s}' for s in ['index.json','extra-plugins/index.json','unknown-origin/index.json','libraries/index.json']}
        need(len({x['path'] for x in indexes})==len(indexes),'Duplicate generated indexes')
        for x in indexes:
            need(x['path'] in expected_paths and HEX.fullmatch(x['sha256']) and x['contents']=={'hashes':{},'skippedHashes':[],'mappingsHash':stem},'Nonempty/wrong generated plugin index')
        route=item['evidence_route']
        need((route=='zero_plugin_startup_log' and zero) or (route=='four_empty_generated_plugin_indexes' and not initialized and {x['path'] for x in indexes}==expected_paths),'Missing exact plugin evidence route')

def check_row(row,evidence_root,binaries,plugin_audit,allow_cleanup_amendment=False):
    version=row['version'];need(version in PROTOCOLS and row['protocol']==PROTOCOLS[version],'Version/protocol mismatch')
    need('error' not in row,'Recorded family error cannot be amended away')
    need(row['passed'] is True or (allow_cleanup_amendment and version=='1.20.6' and row.get('cleanup_failure') is True),'Recorded failure')
    need([t['name'] for t in row['tests']]==['status','registry_tags_probe'],'Missing/duplicated/unexpected client test')
    need(all(t['exit']==0 and t['binary_sha256']==binaries[t['name']] for t in row['tests']),'Failed or mismatched client binary')
    paths={}
    for test in row['tests']:
        rel=Path(test['log']);need(not rel.is_absolute() and '..' not in rel.parts,'Unsafe transcript path')
        path=evidence_root/rel;expect_sha(path,test['log_sha256']);paths[test['name']]=path
        command=test['command']
        need(command[1:]==['127.0.0.1','25565',version]+(['20'] if test['name']=='registry_tags_probe' else []),'Unexpected client target/options')
    status_lines=paths['status'].read_text().splitlines();status=json.loads(status_lines[0])
    need(status['version']['protocol']==PROTOCOLS[version] and any(l.startswith('round trip: ') for l in status_lines),'Status/ping transcript failed')
    need(row['status_nonce_roundtrip'] is True and row['status_version']==status['version'],'Status report disagrees')
    observed=assess_client(paths['registry_tags_probe'].read_text(),version)
    for k,v in observed.items():need(row[k]==v,f'{version}: recorded {k} disagrees with transcript')
    need(20<=row['tests'][1]['seconds']<=90,'Client process duration outside bounds')
    need(all(row[k] is True for k in ['server_clean_disconnect','client_clean_shutdown','joined_server_side','eula_unchanged','ops_empty']),'Missing lifecycle/cleanup evidence')
    need(row['server_exit']==0 and not row['forced_shutdown'] and not row['listeners_after'],'Server cleanup failed')
    need(row['eula_copy_before']==row['eula_copy_after']==row['eula_baseline_after'],'EULA hash mismatch')
    need(row['listeners'] and all(x['address'] in ['0100007F:63DD','0000000000000000FFFF00000100007F:63DD'] for x in row['listeners']),'Non-loopback/missing listener')
    server=evidence_root/'servers'/version/'server.log';expect_sha(server,row['server_log_sha256']);text=server.read_text()
    need('RustwireTags joined the game' in text and 'RustwireTags lost connection: Disconnected' in text,'Server lifecycle transcript missing')
    check_plugin_audit(plugin_audit)
    return observed

def check_all_rows(rows):
    need(len(rows)==14 and {r['version'] for r in rows}==set(VERSIONS),'Need exactly one result for each of 14 families')

def verify_historical(root):
    root=Path(root);check_fixtures(root)
    expect_sha(root/'compact-results.json',COMPACT_SHA);expect_sha(root/'strict-cleanup-assessment.json',ASSESSMENT_SHA)
    compact=read_json(root/'compact-results.json');audit=read_json(root/'strict-cleanup-assessment.json')
    need(compact['source_commit']==COMMIT and compact['source_archive_sha256']==ARCHIVE_SHA and compact['source_tree_manifest_sha256']==SOURCE_MANIFEST_SHA,'Historical source mismatch')
    need(compact['plugin_classifier_sha256']==audit['classifier_sha256']==CLASSIFIER_SHA,'Historical classifier mismatch')
    need(compact['all_14_families_passed'] is True and compact['source_exact_archive_unchanged'] is True and all(v is True for v in compact['cleanup'].values()),'Historical aggregate/cleanup failed')
    first=root/'historical/results.json';remaining=root/'historical/continuation-results.json'
    expect_sha(first,compact['original_results_sha256']);expect_sha(remaining,compact['continuation_results_sha256'])
    a=read_json(first);b=read_json(remaining);rows=a['results']+b['results'];check_all_rows(rows);check_all_rows(compact['versions']);check_all_rows(audit['results'])
    need(a['source_commit']==b['source_commit']==COMMIT and a['binaries']==b['binaries']==compact['binaries'],'Historical build attribution differs')
    need(b['all_passed'] is True and b['source_unchanged'] is True and b['binaries_unchanged'] is True and not b['listeners_final'],'Continuation did not finish cleanly')
    for report in [a,b]:need(report['eula_baseline_before']=={r['version']:r['eula_baseline_after'] for r in rows},'Baseline EULAs changed')
    amended=[]
    for row in rows:
        v=row['version'];assessment=next(x for x in audit['results'] if x['version']==v)
        need(all(assessment[k] is True for k in ['protocol_passed','strict_cleanup_passed','passed']),'Strict cleanup assessment failed')
        need(assessment['original_passed']==row['passed'] and assessment['original_cleanup_failure']==row.get('cleanup_failure',False),'Cleanup amendment loses original outcome')
        check_row(row,root/'historical',a['binaries'],assessment['plugin_safety'],True)
        item=next(x for x in compact['versions'] if x['version']==v)
        need(all(item[k]==row['counters'][k] for k in COUNT_KEYS),'Compact control/chunk counts disagree')
        need(item['roundtrips']==row['roundtrips'] and item['login_seconds']==row['semantic']['elapsed_ms']/1000 and item['tag_members']==row['semantic']['members'] and item['tags']==row['semantic']['tags'] and item['modern_registry_entries']==row['semantic']['modern_registry_entries'],'Compact semantic counts disagree')
        if not row['passed']:amended.append(v)
    need(amended==['1.20.6'],'Unexpected cleanup-only amendment set')
    for key,value in compact['totals'].items():need(value==sum(r[key] for r in compact['versions']),f'Incorrect total {key}')
    return {'families':14,'chunks':compact['totals']['chunks'],'historical_verified':True}

def verify_run(root):
    root=Path(root);verify_source(root);report=read_json(root/'results.json')
    need(report['source_commit']==COMMIT and report['source_archive_sha256']==ARCHIVE_SHA,'Reproduced source attribution differs')
    need(report['plugin_classifier_sha256']==CLASSIFIER_SHA,'Reproduced classifier mismatch')
    need(report['all_passed'] is True and report['source_unchanged'] is True and report['binaries_unchanged'] is True and not report['listeners_final'],'Reproduced run incomplete/failed')
    need(report['eula_baseline_before']==report['eula_baseline_after'],'Baseline EULA changed')
    check_all_rows(report['results'])
    for name,digest in report['binaries'].items():expect_sha(root/'bin'/name,digest)
    for row in report['results']:check_row(row,root,report['binaries'],row['plugin_safety'])
    return {'families':14,'reproduced_run_verified':True,'binary_hashes':report['binaries']}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    group=parser.add_mutually_exclusive_group()
    group.add_argument('--source-only',type=Path,help='Check a prepared fresh run before building/launching')
    group.add_argument('--run-dir',type=Path,help='Accept a finished fresh run; no historical cleanup exception')
    args=parser.parse_args()
    if args.source_only:
        result={'source_verified':verify_source(args.source_only)}
    elif args.run_dir:
        result=verify_run(args.run_dir)
    else:
        result=verify_historical(Path(__file__).resolve().parent)
    print(json.dumps(result,sort_keys=True))

if __name__=='__main__':main()
