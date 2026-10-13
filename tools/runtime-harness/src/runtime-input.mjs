// Portable ingress profile 1. Wire lifecycle/schema versioning is independent.
export const RUNTIME_INPUT_PROFILE=Object.freeze({id:'orchestrix-runtime-json',revision:1});

/** Unicode scalar strings and finite IEEE754 numbers, including ignored fields.
 * This is an iterative lexical guard, not a second JSON parser: JSON.parse still
 * checks the complete syntax. Duplicates with valid values remain last-key-wins.
 */
export function isPortableJson(frame) {
  let quoted=false;
  const hex=offset=>/^[0-9a-fA-F]{4}$/.test(frame.slice(offset,offset+4))?parseInt(frame.slice(offset,offset+4),16):null;
  for(let i=0;i<frame.length;i++) {
    const code=frame.charCodeAt(i),char=frame[i];
    if(quoted) {
      if(char==='"'){quoted=false;continue;}
      if(char==='\\') {
        const escape=frame[++i];
        if(escape==='u') {
          const value=hex(i+1);if(value===null)return false;i+=4;
          if(value>=0xd800&&value<=0xdbff) {
            if(frame.slice(i+1,i+3)!=='\\u')return false;
            const low=hex(i+3);if(low===null||low<0xdc00||low>0xdfff)return false;i+=6;
          } else if(value>=0xdc00&&value<=0xdfff)return false;
        }
        continue;
      }
    } else {
      if(char==='"'){quoted=true;continue;}
      if(char==='-'||code>=48&&code<=57) {
        const start=i;while(i+1<frame.length&&/[0-9eE.+-]/.test(frame[i+1]))i++;
        if(!Number.isFinite(Number(frame.slice(start,i+1))))return false;
        continue;
      }
    }
    if(code>=0xd800&&code<=0xdbff){const low=frame.charCodeAt(++i);if(!(low>=0xdc00&&low<=0xdfff))return false;}
    else if(code>=0xdc00&&code<=0xdfff)return false;
  }
  return true;
}

/** Strict Gregorian/offset input, UTC milliseconds out; never uses host time. */
export function portableTimestamp(value) {
  if(typeof value!=='string'||value.length<20||value.length>35)return null;
  const match=/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?(Z|[+-]\d{2}:\d{2})$/.exec(value);
  if(!match||match[0].length!==value.length)return null;
  const [,rawYear,rawMonth,rawDay,rawHour,rawMinute,rawSecond,fraction='',zone]=match;
  const [year,month,day,hour,minute,second]=[rawYear,rawMonth,rawDay,rawHour,rawMinute,rawSecond].map(Number);
  const leap=year%4===0&&(year%100!==0||year%400===0);
  const days=[31,leap?29:28,31,30,31,30,31,31,30,31,30,31];
  if(month<1||month>12||day<1||day>days[month-1]||hour>23||minute>59||second>59||zone==='-00:00')return null;
  const offsetHour=zone==='Z'?0:Number(zone.slice(1,3)),offsetMinute=zone==='Z'?0:Number(zone.slice(4,6));
  if(offsetHour>23||offsetMinute>59)return null;
  const offset=(offsetHour*60+offsetMinute)*(zone[0]==='-'?-1:1);
  const date=new Date(0);date.setUTCFullYear(year,month-1,day);date.setUTCHours(hour,minute,second,Number(fraction.slice(0,3).padEnd(3,'0')));
  date.setTime(date.getTime()-offset*60000);
  if(date.getUTCFullYear()<0||date.getUTCFullYear()>9999)return null;
  return date.toISOString();
}
