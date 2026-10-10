// Owned offline fixture: no provider, authentication, network or filesystem access.
import {createInterface} from 'node:readline';

const mode=process.argv[2]??'echo';
if(!['echo','blocked'].includes(mode))process.exit(2);
const safety=setTimeout(()=>process.exit(0),8000);
const send=message=>process.stdout.write(`${JSON.stringify(message)}\n`);
send({method:'fixture/ready',params:{mode}});
if(mode==='echo') {
  let received=0,turns=0;
  const input=createInterface({input:process.stdin});
  input.on('line',line=>{
    const message=JSON.parse(line);received++;
    if(!Object.hasOwn(message,'method'))return;
    if(message.method==='turn/start') {
      const turn={id:`fixture-turn-${++turns}`,status:'completed'};
      send({id:message.id,result:{turn:{id:turn.id}}});
      send({method:'turn/completed',params:{threadId:message.params.threadId,turn}});
    } else if(Object.hasOwn(message,'id')) {
      send({id:message.id,result:{received,frameBytes:Buffer.byteLength(`${line}\n`,'utf8')}});
      if(message.method==='fixture/requestPermission')send({id:'owned-request',method:'fixture/permission'});
    }
  });
  input.on('close',()=>clearTimeout(safety));
}
// blocked deliberately never reads stdin; its owner or safety timer ends it.
