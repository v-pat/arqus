import { validateUser } from './validator.ts';
if (validateUser({ email: 'invalid' })) process.exit(1);
if (!validateUser({ email: 'valid@domain.com' })) process.exit(1);
