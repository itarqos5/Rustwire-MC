#!/usr/bin/env python3
"""Create a small, redistributable report without server binaries or worlds."""
import json,pathlib,re,datetime,os
root=pathlib.Path(os.environ.get('RUSTWIRE_VALIDATION_DIR',str(pathlib.Path.cwd()/'.rustwire-validation'))).resolve()
records=json.loads((root/'results.json').read_text())
paper={p['version']:p for p in json.loads((root/'report/download-provenance.json').read_text())['paper']}
summary={'test_date':datetime.datetime.now(datetime.timezone.utc).date().isoformat(),'environment':'isolated cloud Linux process namespace','server_bind':'127.0.0.1:25565','online_mode':False,'max_heap_mib':1024,'minimum_client_runtime_seconds':20,'records':[],'limitations':['No online-account authentication was attempted','Only pinned Paper builds and disposable superflat overworlds were tested','No claim of complete gameplay, plugin, custom dimension, or every protocol-family release coverage','Textual observations are not raw packet captures or replay fixtures']}
observations=[]
for r in sorted(records,key=lambda x:["1.20.1","1.21.1","26.2"].index(x['version'])):
 status=next((t for t in r['tests'] if t['example']=='status'),None)
 client=next((t for t in r['tests'] if t['example']=='offline_chunks'),None)
 row={'version':r['version'],'paper_build':paper[r['version']]['build'],'started_utc':r['started_utc'],'loopback_listener_verified':bool(r.get('listeners')),'server_exit':r['server_exit'],'status_exit':status['exit'] if status else None,'client_exit':client['exit'] if client else None,'client_runtime_seconds':client['seconds'] if client else None}
 if status and status['exit']==0:
  status_json=json.loads(status['output'].splitlines()[0]);row['protocol']=status_json['version']['protocol']
 else:status_json=None
 if client:
  row['client_binary_sha256']=client['client_sha256']
  line=next((l for l in client['output'].splitlines() if l.startswith('validated:')),None)
  row['validation_summary']=line
  dimension=next((l for l in client['output'].splitlines() if l.startswith('joined dimension ')),None)
  if dimension:row['dimension_observation']=dimension
  if not line:row['error']=next((l for l in client['output'].splitlines() if l.startswith('Error:')),None)
  m=re.search(r'validated: (\d+) chunks, (\d+) keepalives, (\d+) teleports, (\d+) registry packets, (\d+) configurations, (\d+) registries',line or '')
  if m:row.update(dict(zip(['decoded_chunks','keepalive_acknowledgements','teleport_acknowledgements','registry_packets','configuration_completions','retained_registries'],map(int,m.groups()))))
  chunklines=[l for l in client['output'].splitlines() if l.startswith('chunk ')][:4]
  lifecycle=[l for l in client['output'].splitlines() if l.startswith(('login ','joined dimension ','configuration complete','teleport acknowledged','validated:','Error:'))]
  observations.append({'version':r['version'],'status_response':status_json,'first_four_decoded_chunk_observations':chunklines,'lifecycle_output':lifecycle})
 summary['records'].append(row)
(root/'report/results-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
(root/'report/textual-observations.json').write_text(json.dumps(observations,indent=2)+'\n')
print(json.dumps(summary,indent=2))
