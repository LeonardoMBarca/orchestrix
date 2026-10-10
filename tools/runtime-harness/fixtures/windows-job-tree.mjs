// Owned, offline processes only. Safety timers bound lifetime after observer loss.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';

const lifetimeMs=20000;
const emit=value=>process.stdout.write(`${JSON.stringify(value)}\n`);
const safetyTimer=setTimeout(()=>process.exit(3),lifetimeMs);

if(process.argv.includes('--descendant')) {
  process.send({ready:true,pid:process.pid});
  process.disconnect();
} else {
  const descendant=spawn(process.execPath,[fileURLToPath(import.meta.url),'--descendant'],{
    stdio:['ignore','inherit','inherit','ipc'],shell:false,windowsHide:true,detached:true
  });
  descendant.on('error',()=>process.exit(2));
  descendant.once('message',message=>{
    if(message?.ready!==true||message.pid!==descendant.pid)process.exit(2);
    emit({type:'fixture.ready',rootPid:process.pid,descendantPid:descendant.pid,lifetimeMs});
    const input=createInterface({input:process.stdin});
    input.on('line',line=>{
      let command;
      try{command=JSON.parse(line);}catch{process.exit(2);}
      if(command.type==='exit-root') {
        clearTimeout(safetyTimer);
        emit({type:'fixture.rootExiting',rootPid:process.pid});
        process.stdout.write('',()=>process.exit(0));
      } else if(command.type==='ping') {
        emit({type:'fixture.pong',rootPid:process.pid});
      } else process.exit(2);
    });
    // EOF alone deliberately keeps both processes alive: job closure must own cleanup.
  });
}
