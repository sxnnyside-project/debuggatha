import { afterEach } from 'vitest';
import { stopHarnesses } from './harness.js';

// Each test starts its own daemon; none outlives it.
afterEach(stopHarnesses);
