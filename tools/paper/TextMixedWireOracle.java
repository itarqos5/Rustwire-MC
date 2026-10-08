// Original Rustwire fixture harness. Invokes pinned release public APIs only.
// No game implementation, server, listener, account sign-in or private data.
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import com.google.common.hash.HashCode;
import com.google.gson.*;
import com.mojang.serialization.JsonOps;
import io.netty.buffer.Unpooled;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.component.DataComponentType;
import net.minecraft.nbt.*;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.util.HashOps;
public class TextMixedWireOracle {
 static final PrintStream REPORT=new PrintStream(new FileOutputStream(FileDescriptor.out),true,StandardCharsets.UTF_8);
 static final HexFormat HEX=HexFormat.of();
 static RegistryAccess registries;
 static RegistryFriendlyByteBuf buf(byte[] input) {
  return new RegistryFriendlyByteBuf(input==null?Unpooled.buffer():Unpooled.wrappedBuffer(input),registries);
 }
 @SuppressWarnings({"unchecked","rawtypes"})
 static int hash(DataComponentType type,Object value) {
  return ((HashCode)type.codec().encodeStart(HashOps.CRC32C_INSTANCE,value).getOrThrow()).asInt();
 }
 @SuppressWarnings({"unchecked","rawtypes"})
 static byte[] wire(DataComponentType type,Object value) {
  var b=buf(null);
  try {type.streamCodec().encode(b,value);byte[] out=new byte[b.readableBytes()];b.readBytes(out);return out;}
  finally {b.release();}
 }
 @SuppressWarnings({"unchecked","rawtypes"})
 static Object decode(DataComponentType type,byte[] bytes) {
  var b=buf(bytes);
  try {var value=type.streamCodec().decode(b);if(b.readableBytes()!=0)throw new IllegalArgumentException("trailing component bytes");return value;}
  finally {b.release();}
 }
 static byte[] nbtWire(Tag tag)throws Exception {
  var bytes=new ByteArrayOutputStream();NbtIo.writeAnyTag(tag,new DataOutputStream(bytes));return bytes.toByteArray();
 }
 @SuppressWarnings({"unchecked","rawtypes"})
 static void emit(String line)throws Exception {
  var f=line.split("\\|",4);var row=new JsonObject();row.addProperty("mode",f[0]);row.addProperty("case",f[1]);row.addProperty("component",f[2]);row.addProperty("input",f[3]);
  try {
   var type=(DataComponentType)DataComponents.class.getField(f[2]).get(null);
   Object value;
   if(f[0].equals("JSON")) value=type.codec().parse(JsonOps.INSTANCE,JsonParser.parseString(f[3])).getOrThrow();
   else if(f[0].equals("WIRE")) value=decode(type,HEX.parseHex(f[3]));
   else throw new IllegalArgumentException("mode");
   int hash=hash(type,value);row.addProperty("hash",hash);
   var canonical=wire(type,value);row.addProperty("canonical_component_wire_hex",HEX.formatHex(canonical));
   row.addProperty("stream_roundtrip_hash",hash(type,decode(type,canonical)));
   Tag tag=(Tag)type.codec().encodeStart(NbtOps.INSTANCE,value).getOrThrow();
   row.addProperty("persistent_nbt_wire_hex",HEX.formatHex(nbtWire(tag)));
   row.addProperty("logical_snbt_diagnostic_only",tag.toString());
   row.add("normalized_json",(JsonElement)type.codec().encodeStart(JsonOps.INSTANCE,value).getOrThrow());
   if(f[2].equals("CUSTOM_NAME")||f[2].equals("ITEM_NAME")) {
    byte[] source=f[0].equals("WIRE")?HEX.parseHex(f[3]):canonical;
    var in=new DataInputStream(new ByteArrayInputStream(source));Tag logical=NbtIo.readAnyTag(in,NbtAccounter.unlimitedHeap());
    if(in.available()!=0)throw new AssertionError("trailing name NBT");
    row.addProperty("nbtio_reencoded_hex",HEX.formatHex(nbtWire(logical)));
    row.addProperty("nbtio_then_persistent_codec_hash",hash(type,type.codec().parse(NbtOps.INSTANCE,logical).getOrThrow()));
   }
  } catch(Exception e) {row.addProperty("error",e.toString());}
  REPORT.println("ORACLE "+row);
 }
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  registries=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
  var input=new BufferedReader(new InputStreamReader(System.in,StandardCharsets.UTF_8));
  for(String line;(line=input.readLine())!=null;)if(!line.isBlank()&&!line.startsWith("#"))emit(line);
 }
}
