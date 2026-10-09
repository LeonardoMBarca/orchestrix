// Offline fixture only. Every descendant expires even if its observer fails.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';

const lifetimeMs=2500;
if(process.argv.includes('--descendant')) {
  setTimeout(()=>process.exit(0),lifetimeMs);
  process.send({ready:true,pid:process.pid});
  process.disconnect();
} else {
  const descendant=spawn(process.execPath,[fileURLToPath(import.meta.url),'--descendant'],{
    stdio:['ignore','inherit','inherit','ipc'],shell:false,windowsHide:true,detached:true
  });
  const safetyTimer=setTimeout(()=>process.exit(2),lifetimeMs+1000);
  descendant.on('error',()=>process.exit(2));
  descendant.once('message',message=>{
    if(message?.ready!==true||message.pid!==descendant.pid)process.exit(2);
    process.stdout.write(`${JSON.stringify({method:'fixture/descendantReady',params:{pid:descendant.pid,lifetimeMs}})}\n`);
    const input=createInterface({input:process.stdin});
    input.on('line',line=>{
      const request=JSON.parse(line);
      if(request.method==='fixture/exitWithUnansweredRequest') {
        clearTimeout(safetyTimer);process.exit(0);
      }
    });
  });
}
