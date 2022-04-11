export interface A {}
export declare function b(): void;
export declare class C {}
export declare enum D {
  A = 0,
  B = 1,
}
export declare const E: string;
export declare type F = string;

export interface GenericI<T1, T2> {}
export declare class GenericC<T1, T2> {}
export declare function genericF<T1, T2>(): void;
export declare type GenericT<T1, T2> = GenericI<T1, T2>;
