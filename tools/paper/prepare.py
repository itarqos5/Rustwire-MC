#!/usr/bin/env python3
"""Download pinned official artifacts; never accept the Minecraft EULA implicitly."""
import argparse,hashlib,json,pathlib,subprocess
parser=argparse.ArgumentParser()
parser.add_argument('--root',type=pathlib.Path,default=pathlib.Path.cwd()/'.rustwire-validation')
parser.add_argument('--manifest',type=pathlib.Path)
args=parser.parse_args();root=args.root.resolve();root.mkdir(parents=True,exist_ok=True)
manifest=json.loads((args.manifest or pathlib.Path(__file__).resolve().parents[2]/'docs/validation/download-provenance.json').read_text())
(root/'report').mkdir(exist_ok=True)
(root/'report/download-provenance.json').write_text(json.dumps(manifest,indent=2)+'\n')
def download(url,dest,sha):
 dest.parent.mkdir(parents=True,exist_ok=True)
 if not dest.exists() or hashlib.sha256(dest.read_bytes()).hexdigest()!=sha:
  subprocess.run(['curl','--fail','--location','--silent','--show-error','--retry','2','--user-agent','Rustwire-Interoperability/0.1 (https://github.com/itarqos5/Rustwire-MC)',url,'--output',str(dest)],check=True)
 actual=hashlib.sha256(dest.read_bytes()).hexdigest()
 if actual!=sha:raise RuntimeError(f'SHA-256 mismatch: {dest}')
 print('Verified',dest.name,actual,flush=True)
for paper in manifest['paper']:
 d=paper['download'];download(d['url'],root/'downloads'/d['name'],d['checksums']['sha256'])
for mojang in manifest['mojang']:
 download(mojang['url'],root/'servers'/mojang['version']/'cache'/mojang['name'],mojang['sha256'])
jdk=manifest['java25']['package'];archive=root/'downloads'/jdk['name']
download(jdk['link'],archive,jdk['checksum'])
(root/'jdk25').mkdir(exist_ok=True)
subprocess.run(['tar','xzf',str(archive),'-C',str(root/'jdk25'),'--strip-components=1'],check=True)
for paper in manifest['paper']:
 version=paper['version'];server=root/'servers'/version;server.mkdir(parents=True,exist_ok=True)
 existing=server/'server.properties'
 if existing.exists():
  old=existing.read_text()
  if not all(line in old.splitlines() for line in ['server-ip=127.0.0.1','online-mode=false','level-name=test-world']):
   raise RuntimeError(f'Refusing to overwrite a non-test server configuration: {existing}')

 eula=server/'eula.txt'
 if not eula.exists():eula.write_text('# Review https://www.minecraft.net/en-us/eula and linked terms before changing this.\neula=false\n')
 props={'server-ip':'127.0.0.1','server-port':'25565','online-mode':'false','enforce-secure-profile':'false','view-distance':'2','simulation-distance':'2','max-players':'2','max-world-size':'64','level-name':'test-world','level-type':'minecraft:flat','generate-structures':'false','spawn-protection':'0','difficulty':'peaceful','gamemode':'creative','enable-rcon':'false','enable-query':'false','enable-jmx-monitoring':'false','sync-chunk-writes':'false','use-native-transport':'false','motd':f'Rustwire isolated Paper {version} interoperability','network-compression-threshold':'256','max-tick-time':'120000','generator-settings':json.dumps({'biome':'minecraft:plains','layers':[{'block':'minecraft:bedrock','height':1},{'block':'minecraft:dirt','height':2},{'block':'minecraft:grass_block','height':1}],'structures':{}})}
 (server/'server.properties').write_text('\n'.join(f'{k}={v}' for k,v in props.items())+'\n')
 (server/'bukkit.yml').write_text('settings:\n  allow-end: false\n  update-folder: update\n  connection-throttle: 0\nspawn-limits:\n  monsters: 0\n  animals: 0\n  water-animals: 0\n  ambient: 0\n')
 (server/'spigot.yml').write_text('settings:\n  bungeecord: false\nworld-settings:\n  default:\n    view-distance: 2\n    simulation-distance: 2\n')
 (server/'config').mkdir(exist_ok=True)
 (server/'config/paper-world-defaults.yml').write_text('_version: 30\nspawn:\n  keep-spawn-loaded: false\n  keep-spawn-loaded-range: 0\n')
(root/'min-runtime-enabled').touch()
print('Prepared. Accept the EULA explicitly before running servers. Nothing was launched.')
