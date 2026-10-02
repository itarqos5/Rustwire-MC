import net.minecraft.util.HashOps;
import com.google.common.hash.HashCode;
import java.util.*;
import java.util.stream.*;
import java.nio.ByteBuffer;
public class HashOracle {
 static void emit(String key, HashCode hash) { System.out.println(key + "=" + hash.asInt()); }
 public static void main(String[] args) {
  var h=HashOps.CRC32C_INSTANCE;
  emit("empty",h.empty()); emit("map_empty",h.emptyMap()); emit("list_empty",h.emptyList());
  emit("byte_negative",h.createByte((byte)-7)); emit("short_negative",h.createShort((short)-1234));
  emit("int_damage3",h.createInt(3)); emit("long_negative",h.createLong(-123456789012L));
  emit("float",h.createFloat(0.2f)); emit("double",h.createDouble(-0.125));
  emit("float_nan",h.createFloat(Float.intBitsToFloat(0x7fc00001))); emit("double_negative_zero",h.createDouble(-0.0));
  emit("string",h.createString("Rustwire \ud83d\ude80")); emit("unpaired",h.createString("\ud800"));
  emit("false",h.createBoolean(false)); emit("true",h.createBoolean(true));
  emit("bytes",h.createByteList(ByteBuffer.wrap(new byte[]{-1,0,1}))); emit("ints",h.createIntList(IntStream.of(-1,0,2147483647))); emit("longs",h.createLongList(LongStream.of(-1,0,Long.MAX_VALUE)));
  emit("list",h.createList(Stream.of(h.createInt(3),h.createString("hello"))));
  emit("custom",h.createMap(Map.of(h.createString("rustwire"),h.createString("probe"),h.createString("nested"),h.createMap(Map.of(h.createString("a"),h.createInt(7))))));
  emit("block_state",h.createMap(Map.of(h.createString("axis"),h.createString("y"))));
  emit("model_default",h.createMap(Map.of()));
  emit("model",h.createMap(Map.of(h.createString("floats"),h.createList(Stream.of(h.createFloat(1.5f))),h.createString("flags"),h.createList(Stream.of(h.createBoolean(true))))));
 }
}
