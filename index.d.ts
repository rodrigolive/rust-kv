/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export class TwoLfu {
  constructor(maxSz: number, batchSize: number, threads: number | undefined | null, path: string)
  set(key: Key, value: any): void
  get(key: Key): any | undefined
  forEach(callback: (...args: any[]) => any): void
  filterKeys(callback: (...args: any[]) => any): unknown[]
  length(): bigint
  startIter(): void
  next(): unknown[] | undefined
  nth(ix: number): unknown[] | undefined
  memory(): number
  entriesIter(): object
  entriesFlat(): Array<string>
  entries2(): unknown[]
  entries(): Array<Array<string>>
  delete(key: Key): void
  destroy(): void
}
export type KVMap = KvMap
export class KvMap {
  constructor()
  set(key: string, value: string): void
  get(key: string): string | undefined
  delete(key: string): void
  startIter(): void
  next(): unknown[] | undefined
}
