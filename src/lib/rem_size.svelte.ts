function parse_rem_size(size: string | undefined): number {
  if (size?.endsWith("px")) {
    return parseInt(size)
  }

  throw new Error("Failed to parse rem size: " + size);
}

export const REM_SIZE = $state({ value: parse_rem_size(document.documentElement.computedStyleMap().get("font-size")?.toString()) });

let observer = new MutationObserver(_ => {
  console.log("MUTATION")

  REM_SIZE.value = parse_rem_size(document.documentElement.computedStyleMap().get("font-size")?.toString());
})

observer.observe(document.documentElement, { attributeFilter: [ "style" ], attributes: true, attributeOldValue: false, characterData: false, characterDataOldValue: false, childList: false, subtree: false });