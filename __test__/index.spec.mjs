import test from 'ava';
import { RustKV } from '../index.js';

test('basics', t => {
    const kv = new RustKV(100_000, 15000, 1, '/tmp/cache1.db');
    const map = {};
    const max = 1_000_000;

    for (let i = 0; i < max; i++) {
        const k = '' + i;
        const v = 'value-' + i;
        kv.set(k, v);
        map[k] = v;
    }

    for (let i = 0; i < max; i++) {
        const k = '' + i;
        const v = kv.get(k);
        t.is(v, map[k]);
    }
    kv.destroy();
});

test('backend data merged with frontend', t => {
    const kv = new RustKV(10_000, 15000, 1, '/tmp/cache1.db');
    const map = {};
    const max = 1_000_000;

    for (let i = 0; i < max; i++) {
        const k = '' + i;
        const v = 'value-' + i;
        kv.set(k, v);
        map[k] = v;
    }

    for (let i = 0; i < max; i++) {
        const k = '' + i;
        const v = kv.get(k);
        t.is(v, map[k]);
    }
    kv.destroy();
});
