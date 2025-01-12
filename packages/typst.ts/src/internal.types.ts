export const kObject = Symbol.for('reflexo-obj');

/**
 * The page information of a Typst document.
 * @property {number} pageOffset - The offset of the page.
 * @property {number} width - The width of the page in pt.
 * @property {number} height - The height of the page in pt.
 */
export class PageInfo {
  pageOffset: number;
  width: number;
  height: number;
}

export interface FsAccessModel {
  getMTime(path: string): Date | undefined;
  isFile(path: string): boolean | undefined;
  getRealPath(path: string): string | undefined;
  readAll(path: string): Uint8Array | undefined;
}

export interface Point {
  x: number;
  y: number;
}

export interface Rect {
  lo: Point;
  hi: Point;
}

export type TransformMatrix = [number, number, number, number, number, number];
