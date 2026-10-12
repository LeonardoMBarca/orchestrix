// Owned synthetic descendant and pressure scenarios. Never launches a provider.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
const mode=process.argv[2]||'descendant';
const safety=setTimeout(()=>process.exit(99),20000);
const emit=message=>process.stdout.write(`${JSON.stringify(message)}\n`);
if(mode==='buffered-final'||mode==='incomplete-utf8') {
  const lines=createInterface({input:process.stdin,crlfDelay:Infinity});
  lines.once('line',line=>{
    const {id}=JSON.parse(line);
    const bytes=mode==='incomplete-utf8'?Buffer.from([0xe2,0x82]):Buffer.from(`${JSON.stringify({id,result:{text:'final 🧩'}})}\n`);
    process.stdout.write(bytes,()=>process.exit(0));
  });
} else if(mode==='blocked-input') {
  emit({method:'fixture/ready',params:{mode}});
  // Deliberately never consume stdin. The helper control channel stays independent.
} else if(mode==='flood-output') {
  emit({method:'fixture/ready',params:{mode}});
  const chunk=Buffer.from(`${JSON.stringify({method:'fixture/data',params:{text:'x'.repeat(32768)}})}\n`);
  function flood(){while(process.stdout.write(chunk)){}process.stdout.once('drain',flood);}
  flood();
} else {
  const descendant=spawn(process.execPath,['-e','setTimeout(()=>process.exit(0),20000)'],{detached:true,windowsHide:true,shell:false,stdio:['ignore','inherit','inherit']});
  descendant.on('error',()=>process.exit(98));
  emit({method:'fixture/descendant',params:{pid:descendant.pid}});
  const lines=createInterface({input:process.stdin,crlfDelay:Infinity});
  lines.on('line',line=>{try{if(JSON.parse(line).method==='fixture/exit-root')process.exit(0);}catch{}});
  process.argv=[process.argv[0],process.argv[1],'--scenario','hang'];
  await import('./fake-app-server.mjs');
}
// Keep the otherwise idle blocked-input process alive for its native deadline.
