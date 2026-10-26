package androidx.compose.runtime
import kotlin.reflect.KProperty
class LongState(var value: Long)
fun mutableLongStateOf(value: Long) = LongState(value)
operator fun LongState.getValue(owner: Any?, property: KProperty<*>) = value
operator fun LongState.setValue(owner: Any?, property: KProperty<*>, value: Long) { this.value = value }
