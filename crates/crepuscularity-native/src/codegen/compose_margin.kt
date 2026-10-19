private fun Modifier.crepusMargin(
    all: Dp = 0.dp,
    horizontal: Dp = all,
    vertical: Dp = all,
    start: Dp = horizontal,
    top: Dp = vertical,
    end: Dp = horizontal,
    bottom: Dp = vertical,
): Modifier {

    data class Margin(
        val start: Dp, val top: Dp, val end: Dp, val bottom: Dp,
    ) : LayoutModifier {
        override fun MeasureScope.measure(measurable: Measurable, constraints: Constraints): MeasureResult {
            val startPx = crepusSignedPx(start)
            val topPx = crepusSignedPx(top)
            val horizontal = startPx.toLong() + crepusSignedPx(end)
            val vertical = topPx.toLong() + crepusSignedPx(bottom)
            // A loose minimum must not grow content just to compensate for negative margins.
            val childConstraints = Constraints.fitPrioritizingWidth(
                minWidth = if (constraints.minWidth == 0) 0 else crepusExtent(constraints.minWidth.toLong() - horizontal),
                maxWidth = if (constraints.maxWidth == Constraints.Infinity) Constraints.Infinity
                    else crepusExtent(constraints.maxWidth.toLong() - horizontal),
                minHeight = if (constraints.minHeight == 0) 0 else crepusExtent(constraints.minHeight.toLong() - vertical),
                maxHeight = if (constraints.maxHeight == Constraints.Infinity) Constraints.Infinity
                    else crepusExtent(constraints.maxHeight.toLong() - vertical),
            )
            val child = measurable.measure(childConstraints)
            val size = Constraints.fitPrioritizingWidth(
                minWidth = constraints.constrainWidth(crepusExtent(child.width.toLong() + horizontal)),
                maxWidth = constraints.constrainWidth(crepusExtent(child.width.toLong() + horizontal)),
                minHeight = constraints.constrainHeight(crepusExtent(child.height.toLong() + vertical)),
                maxHeight = constraints.constrainHeight(crepusExtent(child.height.toLong() + vertical)),
            )
            return layout(size.minWidth, size.minHeight) {
                // placeRelative does not mirror a zero-width slot.
                val x = if (layoutDirection == LayoutDirection.Rtl) {
                    (size.minWidth.toLong() - child.width - startPx)
                        .coerceIn(Int.MIN_VALUE.toLong(), Int.MAX_VALUE.toLong()).toInt()
                } else startPx
                child.place(x, topPx)
            }
        }
    }
    return this.then(Margin(start, top, end, bottom))
}

private fun crepusExtent(value: Long): Int =
    value.coerceIn(0L, Int.MAX_VALUE.toLong()).toInt()

private fun MeasureScope.crepusSignedPx(value: Dp): Int {
    val px = value.toPx()
    // roundToPx maps either signed overflow to Int.MAX_VALUE.
    return when {
        px >= Int.MAX_VALUE.toFloat() -> Int.MAX_VALUE
        px <= Int.MIN_VALUE.toFloat() -> Int.MIN_VALUE
        px.isFinite() -> px.roundToInt()
        else -> 0
    }
}
