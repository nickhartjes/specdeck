import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';
import { setDefaultSchedule } from './model/viewQueue';

afterEach(cleanup);

// jsdom does not lay out or scroll.
Element.prototype.scrollTo = function scrollTo() {};
Element.prototype.scrollIntoView = function scrollIntoView() {};

// Events apply at once, so a test can assert right after emitting; tests of batching pass a
// schedule of their own (`replay-batching` design decision 5).
setDefaultSchedule((flush) => {
  flush();
  return () => {};
});
