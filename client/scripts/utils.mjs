import clc from 'cli-color'

export const log_success = (msg, ...optionalParams) =>
  console.log(clc.green(msg), ...optionalParams)
export const log_error = (msg, ...optionalParams) =>
  console.log(clc.red(msg), ...optionalParams)
export const log_info = (msg, ...optionalParams) =>
  console.log(clc.bgBlue(msg), ...optionalParams)
// A warning is not an error: the build continues, but the reader is told that
// something is not what the repository intends. Used where a failure would be
// wrong (a contributor with no sidecar lock can still build) and silence would
// hide a real risk.
export const log_warn = (msg, ...optionalParams) =>
  console.log(clc.yellow(msg), ...optionalParams)
var debugMsg = clc.xterm(245)
export const log_debug = (msg, ...optionalParams) =>
  console.log(debugMsg(msg), ...optionalParams)
