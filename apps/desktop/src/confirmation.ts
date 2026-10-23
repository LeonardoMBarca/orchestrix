type ConfirmationPrompt=(message:string)=>boolean|Promise<boolean>;

/** Browser confirmation is synchronous; the Tauri dialog plugin replaces it with a Promise. */
export function createConfirmation(show:ConfirmationPrompt,onPending:(pending:boolean)=>void,onError:(error:unknown)=>void){
 let pending=false;
 return async(message:string):Promise<boolean>=>{
  if(pending)return false;
  pending=true;
  onPending(true);
  try{return (await show(message))===true;}
  catch(error){onError(error);return false;}
  finally{pending=false;onPending(false);}
 };
}
