// Synthetic exec --json stream. No provider, auth store, project edits or network.
const args=process.argv.slice(2);
const option=(name,fallback)=>args.includes(name)?args[args.indexOf(name)+1]:fallback;
const scenario=option('--scenario','success');
const threadId=option('--thread-id','00000000-0000-4000-8000-000000000001');
const text=option('--message','Synthetic CLI response — café 🧩');
const send=value=>process.stdout.write(JSON.stringify(value)+'\n');
const thread=()=>send({type:'thread.started',thread_id:threadId});
const start=()=>send({type:'turn.started'});
const message=()=>send({type:'item.completed',item:{id:'item-1',type:'agent_message',text}});
const complete=()=>send({type:'turn.completed',usage:{input_tokens:21,cached_input_tokens:3,output_tokens:8,reasoning_output_tokens:0}});
if(scenario==='malformed'){process.stdout.write('{broken\n');}
else if(scenario==='invalid-utf8'){process.stdout.write(Buffer.from([0xc3,0x28,0x0a]));}
else if(scenario==='incomplete-json'){process.stdout.write('{"type":"thread.started"');}
else if(scenario==='oversized'){process.stdout.write('x'.repeat(2048)+'\n');}
else if(scenario==='empty'){}
else if(scenario==='bad-envelope'){send({type:['turn.started'],secret:'SYNTHETIC_SECRET'});}
else if(scenario==='terminal-before-thread'){complete();}
else if(scenario==='terminal-before-start'){thread();complete();}
else if(scenario==='failed-before-start'){thread();send({type:'turn.failed',error:{message:'SYNTHETIC_SECRET failure'}});process.exitCode=1;}
else if(scenario==='duplicate-thread'){thread();thread();}
else if(scenario==='wrong-thread'){send({type:'thread.started',thread_id:'00000000-0000-4000-8000-000000000099'});}
else if(scenario==='error'){thread();start();send({type:'error',message:'SYNTHETIC_SECRET https://example.invalid?token=private'});process.stderr.write('SYNTHETIC_SECRET stderr');process.exitCode=1;}
else if(scenario==='hang'){thread();start();setInterval(()=>{},1000);}
else if(scenario==='no-start'){thread();setInterval(()=>{},1000);}
else if(scenario==='split-utf8'){
  const bytes=Buffer.from([JSON.stringify({type:'thread.started',thread_id:threadId}),JSON.stringify({type:'turn.started'}),JSON.stringify({type:'item.completed',item:{id:'item-1',type:'agent_message',text}}),JSON.stringify({type:'turn.completed',usage:{input_tokens:21,cached_input_tokens:3,output_tokens:8}})].join('\r\n')+'\r\n');
  for(let index=0;index<bytes.length;index++)process.stdout.write(bytes.subarray(index,index+1));
}
else {
  thread();start();
  if(scenario==='unknown')send({type:'future.event',raw:'SYNTHETIC_SECRET'});
  if(scenario==='noisy-stderr')process.stderr.write('SYNTHETIC_SECRET'.repeat(100000));
  if(scenario==='bad-item')send({type:'item.completed',item:{id:4,type:'agent_message',text}});
  else if(scenario==='contradictory-item'){
    send({type:'item.started',item:{id:'item-1',type:'command_execution',command:'SYNTHETIC_SECRET'}});message();
  } else if(scenario==='unknown-item'){send({type:'item.completed',item:{id:'item-1',type:'__proto__',text:'SYNTHETIC_SECRET'}});}
  else {message();}
  if(scenario==='negative-usage')send({type:'turn.completed',usage:{input_tokens:-1,cached_input_tokens:0,output_tokens:1}});
  else if(scenario==='no-terminal'){}
  else {complete();if(scenario==='duplicate-terminal')complete();}
  if(scenario==='nonzero-exit')process.exitCode=2;
}
