// Authored Rustwire audit: calls public official APIs, copies no game implementation.
import java.io.*;
import java.util.HexFormat;
import net.minecraft.nbt.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.component.DataComponents;
import net.minecraft.util.HashOps;
import net.minecraft.world.item.component.CustomData;
import com.google.common.hash.HashCode;
public class MixedNbtHashOracle {
 public static void main(String[] args) throws Exception {
  final var report=System.out;
  SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
  if (Boolean.TRUE.equals(CustomData.SERIALIZE_CUSTOM_AS_SNBT.get())) throw new IllegalStateException("non-default SNBT mode");
  var input = new BufferedReader(new InputStreamReader(System.in));
  for(String line; (line=input.readLine())!=null;) {
   String[] fields=line.split("\\|",2);
   byte[] bytes=HexFormat.of().parseHex(fields[1]);
   var in=new DataInputStream(new ByteArrayInputStream(bytes));
   Tag tag=NbtIo.readAnyTag(in,NbtAccounter.unlimitedHeap());
   if(in.available()!=0 || !(tag instanceof CompoundTag)) throw new IllegalArgumentException("complete compound required");
   HashCode logical=NbtOps.INSTANCE.convertTo(HashOps.CRC32C_INSTANCE,tag);
   HashCode custom=CustomData.CODEC.encodeStart(HashOps.CRC32C_INSTANCE,CustomData.of((CompoundTag)tag)).getOrThrow();
   var out=new ByteArrayOutputStream(); NbtIo.writeAnyTag(tag,new DataOutputStream(out));
   var data=CustomData.of((CompoundTag)tag);
   report.println("COMPONENTS|"+fields[0]+"|"+DataComponents.CUSTOM_DATA.codec().encodeStart(HashOps.CRC32C_INSTANCE,data).getOrThrow().asInt()+"|"+DataComponents.BUCKET_ENTITY_DATA.codec().encodeStart(HashOps.CRC32C_INSTANCE,data).getOrThrow().asInt());
   report.println("ORACLE|"+fields[0]+"|"+logical.asInt()+"|"+custom.asInt()+"|"+HexFormat.of().formatHex(out.toByteArray())+"|"+tag);
  }
 }
}
