/** OS/auth-location variables only; never API keys, endpoint overrides or NODE_OPTIONS. */
export function runtimeEnvironment(source=process.env) {
  const allowed=new Set(['PATH','PATHEXT','SYSTEMROOT','WINDIR','COMSPEC','TEMP','TMP','TMPDIR','USERPROFILE','HOME','HOMEDRIVE','HOMEPATH','APPDATA','LOCALAPPDATA','PROGRAMDATA','PROGRAMFILES','PROGRAMFILES(X86)','COMMONPROGRAMFILES','PROCESSOR_ARCHITECTURE','NUMBER_OF_PROCESSORS','CODEX_HOME','LANG','LC_ALL']);
  return Object.fromEntries(Object.entries(source).filter(([key,value])=>allowed.has(key.toUpperCase())&&typeof value==='string'));
}
export function fixtureGitEnvironment(source=process.env) {
  return {...runtimeEnvironment(source),GIT_CONFIG_GLOBAL:process.platform==='win32'?'NUL':'/dev/null',GIT_CONFIG_NOSYSTEM:'1',GIT_TERMINAL_PROMPT:'0'};
}
