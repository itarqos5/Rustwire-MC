#!/usr/bin/env python3
"""Download immutable research schemas and verify the recorded SHA-256 hashes."""
import hashlib,json,pathlib,urllib.request
root=pathlib.Path(__file__).resolve().parents[1]
records=json.loads((root/'research/schema-hashes.json').read_text())
dest=root/'research/protocols';dest.mkdir(parents=True,exist_ok=True)
for record in records:
    schema=record['schema']
    if schema=='26.2':repository='Complexity-ML/minecraft-data-26.2';commit='2a6a5fd9ebb0d964a73312d11beef596b7ec029b'
    else:repository='PrismarineJS/minecraft-data';commit='f5d7d74604d8c6153fd086bfe035e0630a5207cc'
    url=f'https://raw.githubusercontent.com/{repository}/{commit}/data/pc/{schema}/protocol.json'
    data=urllib.request.urlopen(url,timeout=60).read()
    if hashlib.sha256(data).hexdigest()!=record['sha256']:raise SystemExit(f'Hash mismatch: {schema}')
    (dest/f'{schema}.json').write_bytes(data)
    print(schema,'verified')
