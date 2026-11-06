import type { Principal } from '../types';

export const SCHEDULER: Principal = { kind: 'scheduler', name: 'scheduler', displayName: 'Scheduler' };
export const SENSOR_DAEMON: Principal = { kind: 'sensor', name: 'sensors', displayName: 'Sensor daemon' };
export const CI_ACCOUNT: Principal = { kind: 'user', name: 'ci', displayName: 'CI deploys' };
export const DEMO_USER: Principal = { kind: 'user', name: 'demo', displayName: 'Demo User' };
export const OPS_USER: Principal = { kind: 'user', name: 'ops-oncall', displayName: 'On-call Operator' };
export const ALEX: Principal = { kind: 'user', name: 'alex', displayName: 'Alex Rivera' };
export const SAM: Principal = { kind: 'user', name: 'sam', displayName: 'Sam Okafor' };
export const JORDAN: Principal = { kind: 'user', name: 'jordan', displayName: 'Jordan Lee' };
export const RILEY: Principal = { kind: 'user', name: 'riley', displayName: 'Riley Chen' };
export const REPORTING: Principal = { kind: 'user', name: 'reporting', displayName: 'Reporting dashboards' };
export const SYSTEM: Principal = { kind: 'system', name: 'system', displayName: 'System' };
