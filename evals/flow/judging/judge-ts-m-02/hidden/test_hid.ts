import * as t from "./test_errors.ts";
if (typeof (t as any).testClientErrors !== 'function') process.exit(1);
(t as any).testClientErrors();
