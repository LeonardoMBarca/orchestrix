import {createHash} from 'node:crypto';
import {mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import {dirname,resolve} from 'node:path';

const root=new URL('../',import.meta.url);
const arguments_=process.argv.slice(2);
if(arguments_.length!==0&&(arguments_.length!==2||arguments_[0]!=='--output')) {
  throw new Error('Usage: node tools/create-manifest.mjs [--output path]');
}
const sha256=bytes=>createHash('sha256').update(bytes).digest('hex');

async function collect(directory,prefix='') {
  const paths=[];
  for(const entry of await readdir(directory,{withFileTypes:true})) {
    const path=prefix+entry.name;
    if(entry.isDirectory())paths.push(...await collect(new URL(entry.name+'/',directory),path+'/'));
    else if(entry.isFile()&&/\.(?:html|css|js|json|png|svg|webp|jpe?g|ico|woff2?)$/.test(entry.name))paths.push(path);
  }
  return paths;
}

const rootFiles=(await readdir(root,{withFileTypes:true}))
  .filter(entry=>entry.isFile()&&(/\.(?:html|css|js)$/.test(entry.name)||entry.name==='video-config.json'))
  .map(entry=>entry.name);
const paths=[...rootFiles,...await collect(new URL('assets/',root),'assets/'),...await collect(new URL('locales/',root),'locales/')]
  .sort((a,b)=>a<b?-1:a>b?1:0);
const files=[];
for(const path of paths) {
  const bytes=await readFile(new URL(path,root));
  files.push({path,bytes:bytes.length,sha256:sha256(bytes)});
}
const manifest={
  schemaVersion:1,
  generatedAt:new Date().toISOString(),
  scope:'D1 static prototype: app, website, video page, help center, local assets and locale sources/catalog; excludes tools, tests, server and documentation',
  algorithm:'SHA-256 of UTF-8 JSON.stringify(files), sorted by relative path; each entry has path, bytes and sha256 of exact file bytes. generatedAt is excluded.',
  buildSha256:sha256(Buffer.from(JSON.stringify(files),'utf8')),
  files,
};
const output=JSON.stringify(manifest,null,2)+'\n';
if(arguments_.length) {
  const destination=resolve(arguments_[1]);
  await mkdir(dirname(destination),{recursive:true});
  await writeFile(destination,output);
  console.log(JSON.stringify({output:destination,files:files.length,buildSha256:manifest.buildSha256}));
} else {
  console.log(output.trimEnd());
}
