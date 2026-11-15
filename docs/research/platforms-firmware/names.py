import ast, json, sys, os
R=sys.argv[1]; G=sys.argv[2]
B=R+'/backend/'
src=open(B+'utils/platform_slugs.py').read()
ups={}
for n in ast.walk(ast.parse(src)):
    if isinstance(n,ast.ClassDef) and n.name=='UniversalPlatformSlug':
        for s in n.body:
            if isinstance(s,ast.Assign): ups[s.targets[0].id]=s.value.value
def load(path,var):
    t=ast.parse(open(B+path).read())
    for n in t.body:
        if isinstance(n,ast.AnnAssign) and getattr(n.target,'id',None)==var:
            out={}
            for k,v in zip(n.value.keys,n.value.values):
                if isinstance(k,ast.Attribute): key=ups[k.attr]
                elif isinstance(k,ast.Constant): key=k.value
                else: continue
                d={}
                if isinstance(v,ast.Dict):
                    for kk,vv in zip(v.keys,v.values):
                        if isinstance(kk,ast.Constant):
                            try: d[kk.value]=ast.literal_eval(vv)
                            except Exception:
                                if isinstance(vv,ast.Attribute) and vv.attr in ups: d[kk.value]=ups[vv.attr]
                                else: d[kk.value]=None
                out[key]=d
            return out
    raise SystemExit('missing '+var+' in '+path)
P=[('igdb','adapters/services/igdb.py','IGDB_PLATFORM_LIST'),
('ss','handler/metadata/ss_handler.py','SCREENSAVER_PLATFORM_LIST'),
('moby','handler/metadata/moby_handler.py','MOBYGAMES_PLATFORM_LIST'),
('ra','handler/metadata/ra_handler.py','RA_PLATFORM_LIST'),
('lb','handler/metadata/launchbox_handler/platforms.py','LAUNCHBOX_PLATFORM_LIST'),
('hasheous','handler/metadata/hasheous_handler.py','HASHEOUS_PLATFORM_LIST'),
('tgdb','handler/metadata/tgdb_handler.py','TGDB_PLATFORM_LIST'),
('fp','handler/metadata/flashpoint_handler.py','FLASHPOINT_PLATFORM_LIST'),
('hltb','handler/metadata/hltb_handler.py','HLTB_PLATFORM_LIST')]
L={p:load(f,v) for p,f,v in P}
VER=load('adapters/services/igdb.py','IGDB_PLATFORM_VERSIONS')
def names(slug):
    out=[]
    for p,_,_ in P:
        if p=='igdb' and slug not in L['igdb'] and slug in VER:
            out.append(('igdb-ver',VER[slug]['name'])); continue
        e=L[p].get(slug)
        if e and e.get('name'): out.append((p,e['name']))
    return out
def resolve(slug):
    n=names(slug)
    return (n[0][1],n[0][0]) if n else (slug.replace('-',' ').title(),'titlecase')
allslugs=set(ups.values())
print('UPS count',len(allslugs), 'igdb',len(L['igdb']),'ss',len(L['ss']),'versions',len(VER))
pe=json.load(open(G+'/platform-emulators.json'))['recommended'].keys()
rc=json.load(open(G+'/romm-platform-cores.json')).keys()
res={}
for s in sorted(set(pe)|set(rc)|{'model3','pico-8','pico8','model2','model1','sega-pico','pico'}):
    nm,src=resolve(s)
    alts=L['ss'].get(s,{}).get('alternative_names',[]) or []
    ig=L['igdb'].get(s,{})
    res[s]=dict(in_ups=s in allslugs, name=nm, src=src, abbr=ig.get('abbreviation',''), igdb_alt=ig.get('alternative_name',''), others=[x for _,x in names(s)][1:])
    print(f"{s}\t{'UPS' if s in allslugs else 'NOT-UPS'}\t{nm}\t[{src}]\tabbr={ig.get('abbreviation','')}\talt={ig.get('alternative_name','')}")
json.dump(res,open(os.path.dirname(sys.argv[0])+'/../names.json','w'),indent=1)
