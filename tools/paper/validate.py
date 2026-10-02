#!/usr/bin/env python3
"""Run a server and its clients in the same isolated network namespace."""
import pathlib,subprocess,time,threading,json,datetime,hashlib,socket,sys,fcntl,os
ROOT=pathlib.Path(os.environ.get('RUSTWIRE_VALIDATION_DIR',str(pathlib.Path.cwd()/'.rustwire-validation'))).resolve()
MANIFEST=json.loads((ROOT/'report/download-provenance.json').read_text())
VERSIONS={p['version']:(p['download']['name'],str(ROOT/'jdk25/bin/java') if p['version'].startswith('26.') else 'java') for p in MANIFEST['paper']}
VERS=sys.argv[1:] or list(VERSIONS)
lock=(ROOT/'one-server.lock').open('w'); fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
BIN=ROOT/'client-bin';BIN.mkdir(exist_ok=True)
for name in ['status','offline_chunks']:
 p=pathlib.Path(os.environ.get('RUSTWIRE_PROJECT',str(pathlib.Path(__file__).resolve().parents[2])))/'target/debug/examples'/name
 (BIN/name).write_bytes(p.read_bytes()); (BIN/name).chmod(0o755)
RESULTS=ROOT/os.environ.get('RUSTWIRE_RESULTS_FILE','results.json')
results=json.loads(RESULTS.read_text()) if RESULTS.exists() else []
for version in VERS:
 jar,java=VERSIONS[version]; wd=ROOT/'servers'/version
 expected=next(p['download']['checksums']['sha256'] for p in MANIFEST['paper'] if p['version']==version)
 if hashlib.sha256((ROOT/'downloads'/jar).read_bytes()).hexdigest()!=expected: raise RuntimeError('Paper artifact checksum changed after preparation')
 assert 'eula=true' in (wd/'eula.txt').read_text().splitlines(), 'Explicit EULA acceptance is required'
 properties=(wd/'server.properties').read_text().splitlines()
 if not all(line in properties for line in ['server-ip=127.0.0.1','online-mode=false','enable-rcon=false','enable-query=false','level-name=test-world']):
  raise RuntimeError('Refusing to launch anything except the isolated loopback test configuration')

 command=[java,'-Xms256M','-Xmx1024M','-XX:ActiveProcessorCount=2','-jar',str(ROOT/'downloads'/jar),'--nogui']
 ready=threading.Event(); serverlog=wd/'validation-server.log'
 print(f'=== START {version} ===',flush=True)
 started=time.monotonic();server=subprocess.Popen(command,cwd=wd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 def output():
  with serverlog.open('w') as log:
   for line in server.stdout:
    log.write(line); log.flush()
    if 'Done (' in line: ready.set(); print(line.strip(),flush=True)
    if any(s in line for s in ['Starting Minecraft server on','This server is running','ERROR','joined the game','lost connection']): print(line.strip(),flush=True)
 reader=threading.Thread(target=output,daemon=True);reader.start()
 record={'version':version,'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'java':java,'server_command':command,'tests':[]}
 try:
  if not ready.wait(150): raise RuntimeError(f'Server failed to start: exit={server.poll()}')
  # Record the actual listening sockets before connecting.
  sockets=[]
  for file in ['/proc/net/tcp','/proc/net/tcp6']:
   for line in pathlib.Path(file).read_text().splitlines()[1:]:
    fields=line.split()
    if fields[1].endswith(':63DD') and fields[3]=='0A': sockets.append(fields[1])
  record['listeners']=sockets;print('Listeners:',sockets,flush=True)
  assert sockets and all(s.startswith('0100007F:') or s.startswith('0000000000000000FFFF00000100007F:') for s in sockets),sockets
  for example in ['status','offline_chunks']:
   clientcmd=[str(BIN/example),'127.0.0.1','25565',version]
   if example=='offline_chunks':clientcmd.extend(['24',os.environ.get('RUSTWIRE_MIN_SECONDS','20')])
   t=time.monotonic()
   try:
    r=subprocess.run(clientcmd,capture_output=True,text=True,timeout=90)
    out=r.stdout+r.stderr;code=r.returncode
   except subprocess.TimeoutExpired as exc:
    out=str(exc);code=-100
   (wd/f'client-{example}.log').write_text(out)
   print(example,'exit',code,'\n'+out,flush=True)
   record['tests'].append({'example':example,'command':clientcmd,'exit':code,'seconds':round(time.monotonic()-t,3),'output':out,'client_sha256':hashlib.sha256((BIN/example).read_bytes()).hexdigest()})
 except Exception as ex:
  record['error']=str(ex);print('FAIL:',ex,flush=True)
 finally:
  if server.poll() is None:
   server.stdin.write('stop\n');server.stdin.flush()
   try: server.wait(timeout=45)
   except subprocess.TimeoutExpired: server.terminate();server.wait(timeout=15)
  reader.join(timeout=5)
  record['server_exit']=server.returncode;record['paper_build']=next(p['build'] for p in MANIFEST['paper'] if p['version']==version);record['paper_channel']=next(p['channel'] for p in MANIFEST['paper'] if p['version']==version);record['seconds']=round(time.monotonic()-started,3)
  results=[r for r in results if r['version']!=version];results.append(record);RESULTS.write_text(json.dumps(results,indent=2))
 print(f'=== STOP {version}: server exit {server.returncode} ===',flush=True)
