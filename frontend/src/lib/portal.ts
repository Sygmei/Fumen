export function portal(node: HTMLElement, target: HTMLElement = document.body) {
  target.appendChild(node)

  return {
    update(nextTarget: HTMLElement = document.body) {
      if (nextTarget !== target) {
        target = nextTarget
        target.appendChild(node)
      }
    },
    destroy() {
      if (node.parentNode) {
        node.parentNode.removeChild(node)
      }
    },
  }
}
