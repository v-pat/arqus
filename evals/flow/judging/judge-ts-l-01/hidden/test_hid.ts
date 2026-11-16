import { Store } from './store.ts';
const s = new Store();
s.dispatch({ type: 'DECREMENT' });
if (s.state !== -1) process.exit(1);
