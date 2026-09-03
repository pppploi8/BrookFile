import type { BookFormat, EngineOptions, ReaderEngine } from '../types'
import { EpubEngine } from './EpubEngine'
import { PdfEngine } from './PdfEngine'
import { TxtEngine } from './TxtEngine'

// 引擎工厂：根据格式创建对应的渲染引擎。
// 后续实现 epub 时，只需在此处新增分支并返回对应引擎，
// 外壳（ReaderShell）与路由层无需任何改动。
export function createEngine(format: BookFormat, opts: EngineOptions): ReaderEngine {
  switch (format) {
    case 'pdf':
      return new PdfEngine(opts)
    case 'txt':
      return new TxtEngine(opts)
    case 'epub':
      return new EpubEngine(opts)
    default: {
      const _exhaustive: never = format
      throw new Error(`Unknown book format: ${String(_exhaustive)}`)
    }
  }
}
