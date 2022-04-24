import { bar } from "./bar"
import { baz } from "./baz"
export * from "./buz"
export type foo = bar
export interface Foo {
   baz: baz
}