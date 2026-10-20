export type WindowPoint={left:number,top:number};
export type WindowSize={width:number,height:number};

// Sizes come from the rendered modal after CSS viewport limits have applied.
// Keep the requested position separate so temporary narrow windows do not
// overwrite the user's last deliberate drag position.
export function containedSettingsPosition(requested:WindowPoint,modal:WindowSize,viewport:WindowSize):WindowPoint{
 return {
  left:Math.max(0,Math.min(Math.max(0,viewport.width-modal.width),requested.left)),
  top:Math.max(0,Math.min(Math.max(0,viewport.height-modal.height),requested.top)),
 };
}
