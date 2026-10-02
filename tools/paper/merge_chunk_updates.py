"""Merge identical frozen codec evidence and transparently reassess cleanup only."""
import collections
import copy
import datetime
import hashlib
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'Rustwire/tools/paper'))
import validate_chunk_updates as probe

ROOT=Path(__file__).resolve().parent
FIRST=ROOT/'runs/20261002T214428.031570Z/chunk-update-results.json'
DEST=probe.PROJECT/'docs/validation/chunk-update-results.json'

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def merge(second):
    first=json.loads(FIRST.read_text()); later=json.loads(second.read_text())
    for key in ['source_base_commit','library_source_sha256','cargo_source_sha256']:
        assert first[key]==later[key], key
    for name in ['examples/chunk_update_probe.rs','tools/paper/validate_gameplay.py']:
        assert first['source_sha256'][name]==later['source_sha256'][name], name
    old=(FIRST.parent/'original-validate_chunk_updates.py').read_text()
    current=(probe.PROJECT/'tools/paper/validate_chunk_updates.py').read_text()
    start=current.index('def audit_plugins(');end=current.index('def run_version(',start)
    normalized=current[:start]+current[end:]
    normalized=normalized.replace('record["plugin_safety"] = audit_plugins(world, [row["line"] for row in server_lines])','record["no_plugin_jars"] = not list((world / "plugins").rglob("*.jar"))').replace('and record["plugin_safety"]["passed"] and','and record["no_plugin_jars"] and')
    assert normalized==old, 'A non-cleanup harness change would invalidate reuse'
    result=copy.deepcopy(later)
    result['scenario']='bounded-chunk-updates-merged'
    result['combined_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat()
    result['merge_script_sha256']=sha(Path(__file__))
    result['merge_policy']='Same library/Cargo/probe/base harness manifests and binary; only documented Paper remap-cache cleanup classification changed. Original results/logs retained. No codec/value acceptance changes.'
    result['runs']=[{'run_id':p.parent.name,'original_report_sha256':sha(p),'created_utc':d['created_utc'],'source_sha256':d['source_sha256'],'versions':[r['version']for r in d['records']]} for p,d in [(FIRST,first),(second,later)]]
    rows=[]
    for document,path in [(first,FIRST),(later,second)]:
        for original in document['records']:
            row=copy.deepcopy(original);world=path.parent/row['version']
            row['source_run_id']=path.parent.name
            row['original_record_sha256']=sha(world/'chunk-update-result.json')
            for name,digest in row['log_sha256'].items():assert sha(world/name)==digest,(row['version'],name)
            assert probe.assess_transcript(row['version'],row['client_output'])==row['value_checks']
            if True:
                log=[probe.base.ANSI.sub('',line).strip() for line in (world/'chunk-update-server.log').read_text().splitlines()]
                audit=probe.audit_plugins(world,log)
                original_plugin_gate=copy.deepcopy(row.get('plugin_safety',row.get('no_plugin_jars')))
                row['plugin_safety']=audit
                accepted=all([row['client_exit']==0,row['server_exit']==0,row['commands_complete'],row['listener_closed'],row['eula_unchanged'],row['no_operators'],row['value_checks']['passed'],not row['console_failures'],audit['passed']])
                row['cleanup_gate_reassessment']={'original_passed':row['passed'],'original_plugin_gate':original_plugin_gate,'amended_passed':accepted,'amended_gate_version':audit['gate_version'],'reason':'Generated Paper remap classpath is not an installed plugin; requires exact cache path, matching mapping and zero-plugin initialization or four exact empty-index evidence.'}
                row['passed']=accepted
            rows.append(row)
    assert len(rows)==14 and {r['version']for r in rows}==set(probe.BUILDS)
    assert len({r['client_binary_sha256']for r in rows})==1
    for row in rows:
        observations=row['value_checks']['observations']
        def matched(category,expected):return any(all(r.get(k)==v for k,v in expected.items())for r in observations.get(category,[]))
        def roundtrip(packet):return matched('wire_roundtrip',{'packet':packet,'semantic':'true'})
        def contextual(packet):return matched('context_dispatch',{'packet':packet,'no_context_raw':'true','contextual':'true'})
        values=probe.required_values(row['version'])
        context_ok=matched('context',values['context']) and not row['value_checks']['protocol_errors']
        status={}
        for packet,category in [('map_chunk','full_chunk'),('update_light','light'),('tile_entity_data','block_entity'),('update_view_position','view_position'),('update_view_distance','view_distance'),('simulation_distance','simulation_distance')]:
            passed=context_ok and matched(category,values[category]) and roundtrip(packet)
            if packet in ['map_chunk','update_light']:passed &= contextual(packet)
            if packet=='map_chunk':passed &= matched('embedded_sign',values['embedded_sign'])
            status[packet]='verified' if passed else 'unverified'
        biomes=all(matched('biomes',{'stage':stage,'x':'0','z':'0','sections':'24','desert':desert,'plains':plains,'exact_pattern':'true'})for stage,desert,plains in [('biome_desert','1536','0'),('biome_mixed','1520','16')])
        status['chunk_biomes']='verified' if context_ok and biomes and roundtrip('chunk_biomes') and contextual('chunk_biomes') else 'unverified'
        row['surface_verification']=status
        row['additional_unverified']=['No standalone UpdateLight packet was emitted in this bounded run; other surfaces were independently verified.'] if status['update_light']=='unverified' else []
    result['records']=sorted(rows,key=lambda r:r['protocol'])
    result['amended_cleanup_gate_source_sha256']=sha(probe.PROJECT/'tools/paper/validate_chunk_updates.py')
    result['amended_cleanup_gate_version']='paper-generated-remap-classpath-v2'
    result['amended_cleanup_gate_test_source_sha256']=sha(probe.PROJECT/'tools/paper/test_validate_chunk_updates.py')
    counts=collections.Counter(); exact=canonical=0
    for row in rows:
        for r in row['value_checks']['observations']['wire_roundtrip']:
            counts[r['packet']]+=1
            exact+=r['equal']=='true';canonical+=r['equal']=='false'
    result['summary']={'versions_tested':len(rows),'versions_fully_passed':sum(r['passed']for r in rows),'per_surface_verified_versions':{packet:sum(r['surface_verification'][packet]=='verified'for r in rows)for packet in probe.ROUNDTRIPS},'partial_versions':[{'version':r['version'],'unverified_surfaces':[p for p,v in r['surface_verification'].items()if v=='unverified'],'missing_value_groups':r['value_checks']['missing_or_wrong']}for r in rows if not r['passed']],'unverified_value_check_groups':sum(len(r['value_checks']['missing_or_wrong'])for r in rows),'required_value_check_groups':sum(len(probe.required_values(r['version']))+2+len(probe.ROUNDTRIPS)+3 for r in rows),'roundtrip_packet_counts':dict(sorted(counts.items())),'byte_exact_roundtrips':exact,'canonicalized_semantic_roundtrips':canonical,'dynamic_light_transition_versions':[r['version']for r in rows if r['value_checks']['dynamic_light_transition_observed']],'protocol_errors':sum(len(r['value_checks']['protocol_errors'])for r in rows),'console_failures':sum(len(r['console_failures'])for r in rows),'all_clients_and_servers_exit_zero':all(r['client_exit']==0 and r['server_exit']==0 for r in rows),'all_listeners_closed':all(r['listener_closed']for r in rows),'all_eulas_unchanged':all(r['eula_unchanged']for r in rows),'all_no_operators':all(r['no_operators']for r in rows),'all_plugin_safety_passed':all(r['plugin_safety']['passed']for r in rows)}
    DEST.write_text(json.dumps(result,indent=2)+'\n')
    (ROOT/'combined-results.json').write_bytes(DEST.read_bytes())
    print(json.dumps({'report':str(DEST),'sha256':sha(DEST),'binary_sha256':rows[0]['client_binary_sha256'],'summary':result['summary']},indent=2))

if __name__=='__main__':merge(Path(sys.argv[1]))
