import { loadData } from "./api.ts";
loadData("3").then(res => { if (res !== "data-3") process.exit(1); });
