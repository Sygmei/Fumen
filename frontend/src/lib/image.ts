export async function compressImageToJpeg(
  file: File,
  size = 256,
  quality = 0.88,
): Promise<File> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => {
      const canvas = document.createElement('canvas')
      canvas.width = size
      canvas.height = size
      const ctx = canvas.getContext('2d')!
      const srcSize = Math.min(img.naturalWidth, img.naturalHeight)
      const sx = (img.naturalWidth - srcSize) / 2
      const sy = (img.naturalHeight - srcSize) / 2
      ctx.drawImage(img, sx, sy, srcSize, srcSize, 0, 0, size, size)
      canvas.toBlob(
        (blob) => {
          if (!blob) {
            reject(new Error('Canvas toBlob returned null'))
            return
          }
          resolve(new File([blob], 'avatar.jpg', { type: 'image/jpeg' }))
        },
        'image/jpeg',
        quality,
      )
      URL.revokeObjectURL(img.src)
    }
    img.onerror = () => reject(new Error('Failed to load image'))
    img.src = URL.createObjectURL(file)
  })
}
