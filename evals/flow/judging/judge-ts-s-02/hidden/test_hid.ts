import { deepClone } from "./clone.ts";
const res = deepClone({ items: [1, 2, 3] });
if (!Array.isArray(res.items) || res.items.length !== 3) process.exit(1);
