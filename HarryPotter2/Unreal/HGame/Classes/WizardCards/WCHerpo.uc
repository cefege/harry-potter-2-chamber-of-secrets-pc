//===============================================================================
//  #11 Herpo the Foul: Creator of the Basilisk
//===============================================================================

class  WCHerpo extends Goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardHerpoTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\herposmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardHerpoBigTexture FILE=TEXTURES\menu\Folio\Cards\Herpobig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardHerpoBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\11_Herpo_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardHerpoBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\11_Herpo_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardHerpoBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\11_Herpo_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Herpo!";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=11
    skin(0)=Texture'HProps.skins.WizardCardHerpoTex0'
	textureBig=Texture'WizCardHerpoBigTexture'
	strDescriptionId="WizCard_0049"

	bIsLayered=true;
	textureLayers(0)=Texture'WizCardHerpoBigTextureLayer0';
	textureLayers(1)=Texture'WizCardHerpoBigTextureLayer1';
	textureLayers(2)=Texture'WizCardHerpoBigTextureLayer2';


}

	
/*	GoldCard3DList(0)=(number=15,firstName="Paracelsus");
	GoldCard3DList(1)=(number=40,firstName="Carlotta");
	GoldCard3DList(2)=(number=74,firstName="Montague");
	GoldCard3DList(3)=(number=41,firstName="Godric");
	GoldCard3DList(4)=(number=82,firstName="Rowena");
	GoldCard3DList(5)=(number=48,firstName="Salazar");
	GoldCard3DList(6)=(number=69,firstName="Bertie");
	GoldCard3DList(7)=(number=72,firstName="Helga");
	GoldCard3DList(8)=(number=101,firstName="Dumbledore");
	GoldCard3DList(9)=(number=11,firstName="Herpo");
	GoldCard3DList(10)=(number=100,firstName="Harry");

*/