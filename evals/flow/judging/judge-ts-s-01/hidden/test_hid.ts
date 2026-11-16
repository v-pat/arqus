import { slugify } from "./slug.ts";
if (slugify("  Special & Chars! ") !== "special-chars") process.exit(1);
