# Authored synthetic NBT inputs. The release oracle determines acceptance/hashes.
import json,struct,pathlib
S=lambda s:struct.pack('>H',len(s.encode()))+s.encode()
def string(s):return (8,S(s))
def byte(n):return (1,struct.pack('b',n))
def integer(n):return (3,struct.pack('>i',n))
def compound(*entries):return (10,b''.join(bytes([v[0]])+S(k)+v[1] for k,v in entries)+b'\0')
def nlist(kind,*values):
 assert all(v[0]==kind for v in values)
 return (9,bytes([kind])+struct.pack('>i',len(values))+b''.join(v[1] for v in values))
def wire(tag):return bytes([tag[0]])+tag[1]
def wrapper(tag):return compound(('',tag))
a=string('a');b=compound(('text',string('b')),('bold',byte(1)))
root=nlist(10,wrapper(a),b)
extra=compound(('text',string('x')),('extra',root))
cases={
'root_mixed_list':root,
'extra_mixed_list':extra,
'root_nested_mixed_list':nlist(10,wrapper(root),wrapper(string('c'))),
'extra_nested_mixed_list':compound(('text',string('x')),('extra',nlist(10,wrapper(root),wrapper(string('c'))))),
'root_single_wrapper':nlist(10,wrapper(a)),
'root_duplicate_empty_key':nlist(10,compound(('',integer(7)),('',a)),b),
'extra_duplicate_empty_key':compound(('text',string('x')),('extra',nlist(10,compound(('',integer(7)),('',a)),b))),
'root_double_wrapper_reject':nlist(10,wrapper(wrapper(a)),b),
'extra_double_wrapper_reject':compound(('text',string('x')),('extra',nlist(10,wrapper(wrapper(a)),b))),
'root_empty_key_compound_reject':wrapper(a),
'root_literal_wrapper':nlist(10,wrapper(compound(('text',a))),b),
'root_empty_plus_text_control':nlist(10,compound(('',string('ignored')),('text',a)),b),
'root_duplicate_empty_plus_text_control':nlist(10,compound(('',string('first')),('text',a),('',string('last'))),b),
'root_ordinary_compound_list':nlist(10,compound(('text',a)),b),
'root_string_list_control':nlist(8,a,string('b')),
'extra_empty_compound_reject':compound(('text',string('x')),('extra',nlist(10,compound(),b))),
}
main={'text':'x','extra':['a',{'text':'b','bold':True}]}
rows=[]
for comp in ['CUSTOM_NAME','ITEM_NAME']:
 for name,value in [('mixed_literal',main),('root_list',['a',{'text':'b','bold':True}]),('root_nested_list',[['a',{'text':'b','bold':True}],'c']),('nested_extra',{'text':'x','extra':[['a',{'text':'b','bold':True}],'c']}),('plain_control','plain'),('homogeneous_compounds',{'text':'x','extra':[{'text':'a','bold':False},{'text':'b','bold':True}]})]:
  rows.append('|'.join(['JSON',comp.lower()+'_'+name,comp,json.dumps(value,separators=(',',':'))]))
 for name,tag in cases.items():rows.append('|'.join(['WIRE',comp.lower()+'_'+name,comp,wire(tag).hex()]))
rows.append('|'.join(['JSON','lore_mixed','LORE',json.dumps([main,{'text':'tail','italic':False}],separators=(',',':'))]))
rows.append('|'.join(['WIRE','lore_raw_root_mixed','LORE',(b'\x02'+wire(root)+wire(extra)).hex()]))
book={'title':{'raw':'T','filtered':'Clean'},'author':'A','generation':2,'pages':[main,{'raw':['a',{'text':'b','bold':True}],'filtered':main}],'resolved':True}
rows.append('|'.join(['JSON','written_book_mixed','WRITTEN_BOOK_CONTENT',json.dumps(book,separators=(',',':'))]))
pathlib.Path('inputs.tsv').write_text('\n'.join(rows)+'\n')
