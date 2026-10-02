// Original Rustwire test harness. Requires an independently obtained official server install.
// Generates interoperability facts only; no proprietary implementation is reproduced.
import net.minecraft.util.HashOps;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.component.DataComponentType;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import com.google.common.hash.HashCode;
import com.mojang.serialization.*;
import java.util.*;
public class ComponentHashOracle {
 static void emit(String name, DataComponentType type, String json) {
  var value=type.codec().parse(JsonOps.INSTANCE,com.google.gson.JsonParser.parseString(json)).getOrThrow();
  var code=(HashCode)type.codec().encodeStart(HashOps.CRC32C_INSTANCE,value).getOrThrow();
  System.out.println("HASH " + name + "=" + code.asInt());
 }
 public static void main(String[] args) throws Exception {
  SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
  var data=net.minecraft.world.item.component.CustomData.of(net.minecraft.nbt.TagParser.parseCompoundFully("{rustwire:\"probe\",nested:{a:7}}"));
  System.out.println("HASH custom_data_snbt=" + DataComponents.CUSTOM_DATA.codec().encodeStart(HashOps.CRC32C_INSTANCE,data).getOrThrow().asInt());
  emit("damage3",DataComponents.DAMAGE,"3");
  emit("unit",DataComponents.UNBREAKABLE,"{}");
  emit("rarity",DataComponents.RARITY,"\"rare\"");
  emit("custom_data",DataComponents.CUSTOM_DATA,"{\"rustwire\":\"probe\",\"nested\":{\"a\":7}}");
  emit("block_state",DataComponents.BLOCK_STATE,"{\"axis\":\"y\"}");
  emit("custom_model_default",DataComponents.CUSTOM_MODEL_DATA,"{}");
  emit("custom_model",DataComponents.CUSTOM_MODEL_DATA,"{\"floats\":[1.5],\"flags\":[true]}");
  emit("tooltip_default",DataComponents.TOOLTIP_DISPLAY,"{}");
  emit("enchantable",DataComponents.ENCHANTABLE,"{\"value\":10}");
 }
}
