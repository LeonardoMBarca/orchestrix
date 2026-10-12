export class HarnessError extends Error {
  constructor(kind,message,metadata={}) {
    super(message);this.name='HarnessError';this.kind=kind;this.metadata=metadata;
  }
}
