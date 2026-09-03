import * as pdfjsLib from 'pdfjs-dist'
import PdfWorker from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

// 复用 reader 的 pdf.js 初始化方式（Vite 将 worker 作为静态资源处理）。
pdfjsLib.GlobalWorkerOptions.workerSrc = PdfWorker

/**
 * 在浏览器端把 PDF 第一页渲染成压缩后的封面图。
 * 返回 JPEG Blob（已限制最大宽度并压缩），失败时返回 null。
 * 这样 PDF 封面无需后端依赖任何原生库，统一由前端生成后回传。
 */
export async function renderPdfFirstPageCover(
  buf: ArrayBuffer,
  maxWidth = 300,
): Promise<Blob | null> {
  try {
    const doc = await pdfjsLib.getDocument({ data: new Uint8Array(buf) }).promise
    const page = await doc.getPage(1)
    const baseViewport = page.getViewport({ scale: 1 })
    const scale = maxWidth / baseViewport.width
    const viewport = page.getViewport({ scale })

    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.ceil(viewport.width))
    canvas.height = Math.max(1, Math.ceil(viewport.height))
    const ctx = canvas.getContext('2d')
    if (!ctx) return null

    await page.render({ canvasContext: ctx, viewport }).promise

    const blob = await new Promise<Blob | null>((resolve) =>
      canvas.toBlob((b) => resolve(b), 'image/jpeg', 0.82),
    )
    return blob
  } catch {
    return null
  }
}
